use convert_case::Casing;
use juniper::meta::{Argument, Field};
use juniper::{
	Arguments, BoxFuture, ExecutionResult, InputValue, IntoFieldError, Object, Registry,
	ScalarValue, Value,
};
use rust_arango::{AqlQuery, ClientError};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::api::schema::errors::{DatabaseError, NotFoundError};
use crate::api::schema::operations::create::Create;
use crate::api::schema::operations::get::Get;
use crate::api::schema::operations::get_all::GetAll;
use crate::api::schema::operations::remove::Remove;
use crate::api::schema::operations::remove_all::RemoveAll;
use crate::api::schema::operations::update::Update;
use crate::api::schema::operations::update_all::UpdateAll;
use crate::api::schema::{AsyncScalarValue, SchemaKind};
use crate::lib::database::api::{DbEntity, DbRelationship};
use crate::lib::database::aql::{
	AQLFilterOperation, AQLLogicalFilter, AQLLogicalOperator, AQLNode, AQLOperation, AQLQuery,
	AQLQueryBind, AQLQueryParameter,
};
use crate::lib::database::DATABASE;

pub mod utils;

pub mod create;
pub mod get;
pub mod get_all;
pub mod remove;
pub mod remove_all;
pub mod update;
pub mod update_all;

type FutureType<'b, S> = BoxFuture<'b, ExecutionResult<S>>;

pub struct OperationRegistry<S>
where
	S: AsyncScalarValue,
{
	operation_data: HashMap<String, Arc<OperationData<S>>>,
	operations: HashMap<String, OperationEntry<S>>,
}

pub struct OperationEntry<S>
where
	S: AsyncScalarValue,
{
	pub closure: for<'a> fn(&'a OperationData<S>, &'a Arguments<S>, AQLQuery) -> FutureType<'a, S>,
	pub arguments_closure: for<'a> fn(
		&mut Registry<'a, S>,
		data: &OperationData<S>,
		&OperationRegistry<S>,
	) -> Vec<Argument<'a, S>>,
	pub field_closure: for<'a> fn(
		&mut Registry<'a, S>,
		name: &str,
		data: &OperationData<S>,
		&OperationRegistry<S>,
	) -> Field<'a, S>,

	pub data: Arc<OperationData<S>>,
	pub kind: SchemaKind,
}

impl<S> OperationRegistry<S>
where
	S: AsyncScalarValue,
{
	pub fn new() -> OperationRegistry<S> {
		OperationRegistry {
			operation_data: HashMap::new(),
			operations: HashMap::new(),
		}
	}

	pub fn call_by_key<'b>(
		&'b self,
		key: &str,
		arguments: &'b Arguments<S>,
		query: AQLQuery,
	) -> Option<FutureType<'b, S>> {
		self.operations
			.get(key)
			.map(|o| (o.closure)(&o.data, arguments, query))
	}

	pub fn get_operations(&self, kind: SchemaKind) -> HashMap<&String, &OperationEntry<S>> {
		self.operations
			.iter()
			.filter(|(_, entry)| entry.kind == kind)
			.collect()
	}

	pub fn get_operation(&self, key: &str) -> Option<&OperationEntry<S>> {
		self.operations.get(key)
	}

	pub fn get_operation_data(&self, key: &str) -> Option<Arc<OperationData<S>>> {
		self.operation_data.get(key).map(|e| e.clone())
	}

	pub fn register_entity(&mut self, entity: Arc<DbEntity>, relationships: Vec<DbRelationship>) {
		let data = Arc::new(OperationData {
			entity: entity.clone(),
			relationships,

			_phantom: Default::default(),
		});

		self.operation_data
			.insert(entity.name.clone(), data.clone());

		vec![
			self.register::<Get>(data.clone(), SchemaKind::Query),
			self.register::<GetAll>(data.clone(), SchemaKind::Query),
			self.register::<Update>(data.clone(), SchemaKind::Mutation),
			self.register::<UpdateAll>(data.clone(), SchemaKind::Mutation),
			self.register::<Remove>(data.clone(), SchemaKind::Mutation),
			self.register::<RemoveAll>(data.clone(), SchemaKind::Mutation),
			self.register::<Create>(data.clone(), SchemaKind::Mutation),
		];
	}

	fn register<T: 'static>(&mut self, data: Arc<OperationData<S>>, kind: SchemaKind) -> String
	where
		T: Operation<S>,
	{
		let k = T::get_operation_name(&data);

		self.operations.insert(
			k.clone(),
			OperationEntry {
				closure: T::call,
				arguments_closure: T::get_arguments,
				field_closure: T::build_field,
				data,
				kind,
			},
		);

		k
	}
}

pub struct OperationData<S>
where
	S: ScalarValue,
{
	pub entity: Arc<DbEntity>,
	pub relationships: Vec<DbRelationship>,

	_phantom: PhantomData<S>,
}

pub trait Operation<S>
where
	S: AsyncScalarValue,
	Self: Send + Sync,
{
	fn call<'b>(
		data: &'b OperationData<S>,
		arguments: &'b Arguments<S>,
		query: AQLQuery,
	) -> FutureType<'b, S>;

	fn get_operation_name(data: &OperationData<S>) -> String;

	fn get_arguments<'r, 'd>(
		registry: &mut Registry<'r, S>,
		data: &'d OperationData<S>,
		operation_registry: &OperationRegistry<S>,
	) -> Vec<Argument<'r, S>>;

	fn build_field<'r>(
		registry: &mut Registry<'r, S>,
		name: &str,
		data: &OperationData<S>,
		operation_registry: &OperationRegistry<S>,
	) -> Field<'r, S>;

	fn get_relationship_edge_name(relationship: &DbRelationship) -> String {
		format!(
			"{}_{}",
			pluralizer::pluralize(
				relationship
					.from
					.name
					.to_case(convert_case::Case::Snake)
					.as_str(),
				1,
				false,
			),
			relationship.name
		)
	}
}

fn convert_number<S>(n: &JsonNumber) -> Value<S>
where
	S: AsyncScalarValue,
{
	return if n.is_i64() {
		let v = n.as_i64().unwrap();

		let res = if v > i32::MAX as i64 {
			i32::MAX
		} else if v < i32::MIN as i64 {
			i32::MIN
		} else {
			v as i32
		};

		Value::scalar(res)
	} else if n.is_u64() {
		let v = n.as_u64().unwrap();

		let res = if v > i32::MAX as u64 {
			i32::MAX
		} else if v < i32::MIN as u64 {
			i32::MIN
		} else {
			v as i32
		};

		Value::scalar(res)
	} else {
		let v = n.as_f64().unwrap();

		Value::scalar(v)
	};
}

fn convert_json_to_juniper_value<S>(data: &JsonMap<String, JsonValue>) -> Value<S>
where
	S: AsyncScalarValue,
{
	let mut object = Object::<S>::with_capacity(data.len());

	fn convert<S>(val: &JsonValue) -> Value<S>
	where
		S: AsyncScalarValue,
	{
		match val {
			JsonValue::Null => Value::null(),
			JsonValue::Bool(v) => Value::scalar(v.to_owned()),
			JsonValue::Number(n) => convert_number(n),
			JsonValue::String(s) => Value::scalar(s.to_owned()),
			JsonValue::Array(a) => Value::list(a.iter().map(|i| convert(i)).collect()),
			JsonValue::Object(ref o) => convert_json_to_juniper_value(o),
		}
	}

	for (key, val) in data {
		object.add_field(key, convert(val));
	}

	Value::Object(object)
}

fn get_filter_by_indices_attributes<S>(
	attributes: &HashMap<String, InputValue<S>>,
) -> Box<dyn AQLNode>
where
	S: AsyncScalarValue,
{
	let mut nodes: Vec<Box<dyn AQLNode>> = Vec::new();

	for (key, _) in attributes {
		nodes.push(Box::new(AQLFilterOperation {
			left_node: Box::new(AQLQueryParameter(key.clone())),
			operation: AQLOperation::Equal,
			right_node: Box::new(AQLQueryBind(key.clone())),
		}));
	}

	Box::new(AQLLogicalFilter {
		operation: AQLLogicalOperator::AND,
		nodes,
	})
}

fn get_single_entry<S>(
	entries: Result<Vec<JsonValue>, ClientError>,
	entity_name: String,
) -> ExecutionResult<S>
where
	S: AsyncScalarValue,
{
	let not_found_error = NotFoundError::new(entity_name).into_field_error();

	return match entries {
		Ok(data) => {
			if let Some(first) = data.first() {
				let time = std::time::Instant::now();

				let ret = Ok(convert_json_to_juniper_value(first.as_object().unwrap()));

				println!("Conversion: {:?}", time.elapsed());

				return ret;
			}

			Err(not_found_error)
		}
		Err(e) => {
			let message = format!("{}", e);

			Err(DatabaseError::new(message).into_field_error())
		}
	};
}

fn get_multiple_entries<S>(entries: Result<Vec<JsonValue>, ClientError>) -> ExecutionResult<S>
where
	S: AsyncScalarValue,
{
	return match entries {
		Ok(data) => {
			let mut output = Vec::<Value<S>>::new();

			let time = std::time::Instant::now();

			for datum in data {
				output.push(convert_json_to_juniper_value(datum.as_object().unwrap()));
			}

			println!("Output conversion: {:?}", time.elapsed());

			Ok(Value::list(output))
		}
		Err(e) => {
			let message = format!("{}", e);

			Err(DatabaseError::new(message).into_field_error())
		}
	};
}

pub enum QueryReturnType {
	Single,
	Multiple,
}

async fn execute_internal_query<S>(
	query: AQLQuery,
	collection: &str,
	query_arguments: HashMap<String, InputValue<S>>,
) -> Vec<JsonValue>
where
	S: AsyncScalarValue,
{
	let time = std::time::Instant::now();

	let aql = query.to_aql();

	println!("Internal Query: {}", &aql);

	let mut entries_query = AqlQuery::builder()
		.query(&aql)
		.bind_var("@collection".to_string(), collection.clone());

	for (key, value) in query_arguments {
		match value {
			InputValue::Scalar(s) => {
				if let Some(int) = s.as_int() {
					entries_query =
						entries_query.bind_var(query.get_argument_key(key.as_str()), int);
				} else if let Some(float) = s.as_float() {
					entries_query =
						entries_query.bind_var(query.get_argument_key(key.as_str()), float);
				} else if let Some(str) = s.as_string() {
					entries_query =
						entries_query.bind_var(query.get_argument_key(key.as_str()), str);
				}
			}
			_ => {
				println!(
					"WARN: Using non-scalar for query arguments ({}, {})",
					key,
					value.to_string()
				)
			}
		}
	}

	let entries: Result<Vec<JsonValue>, ClientError> = DATABASE
		.get()
		.await
		.database
		.aql_query(entries_query.build())
		.await;

	println!("Internal Query AQL: {:?}", time.elapsed());

	entries.unwrap()
}

async fn execute_query<'a, S>(
	query: AQLQuery,
	entity: &'a DbEntity,
	collection: &'a str,
	return_type: QueryReturnType,
	query_arguments: HashMap<String, InputValue<S>>,
) -> ExecutionResult<S>
where
	S: AsyncScalarValue,
{
	let time = std::time::Instant::now();

	let query_str = query.to_aql();

	println!("{}", &query_str);

	let mut entries_query = AqlQuery::builder()
		.query(&query_str)
		.bind_var("@collection".to_string(), collection.clone());

	for (key, value) in query_arguments {
		match value {
			InputValue::Scalar(s) => {
				if let Some(int) = s.as_int() {
					entries_query =
						entries_query.bind_var(query.get_argument_key(key.as_str()), int);
				} else if let Some(float) = s.as_float() {
					entries_query =
						entries_query.bind_var(query.get_argument_key(key.as_str()), float);
				} else if let Some(str) = s.as_string() {
					entries_query =
						entries_query.bind_var(query.get_argument_key(key.as_str()), str);
				}
			}
			_ => {
				println!(
					"WARN: Using non-scalar for query arguments ({}, {})",
					key,
					value.to_string()
				)
			}
		}
	}

	let entries: Result<Vec<JsonValue>, ClientError> = DATABASE
		.get()
		.await
		.database
		.aql_query(entries_query.build())
		.await;

	println!("SQL: {:?}", time.elapsed());

	match return_type {
		QueryReturnType::Single => get_single_entry(entries, entity.name.clone()),
		QueryReturnType::Multiple => get_multiple_entries(entries),
	}
}

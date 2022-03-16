pub mod operations;

use juniper::meta::{Field, MetaType};
use juniper::{
	Arguments, BoxFuture, DefaultScalarValue, EmptyMutation, EmptySubscription, ExecutionResult,
	Executor, GraphQLType, GraphQLValue, GraphQLValueAsync, Registry, RootNode, ScalarValue
};
use crate::api::schema::operations::OPERATION_REGISTRY;

use crate::lib::database::api::GraphQLType as ApiGraphQLType;
use crate::lib::database::api::*;

pub type Schema = RootNode<'static, Query, EmptyMutation, EmptySubscription>;

pub fn schema(map: GraphQLMap) -> Schema {
	let mut types: Vec<ApiGraphQLType> = Vec::new();

	for p in map.0 {
		match p {
			GraphQLPrimitive::Type(t) => types.push(*t),
			GraphQLPrimitive::Enum(_) => {}
		}
	}

	RootNode::new_with_info(
		Query,
		EmptyMutation::new(),
		EmptySubscription::new(),
		types,
		(),
		(),
	)
}

pub struct Query;

impl<S> GraphQLType<S> for Query
where
	S: ScalarValue,
{
	fn name(_: &Self::TypeInfo) -> Option<&str> {
		Some("Query")
	}

	fn meta<'r>(info: &Self::TypeInfo, registry: &mut Registry<'r, S>) -> MetaType<'r, S>
	where
		S: 'r,
	{
		let mut queries = Vec::new();

		for gql_type in info {
			let registered_operations = OPERATION_REGISTRY.lock().unwrap().register_entity(&gql_type.name);

			for registered_operation in registered_operations {
				if let Some(operation) = registered_operation {
					queries.push(registry.field::<QueryField>(operation.as_str(), gql_type));
				}
			}
		}

		registry
			.build_object_type::<Query>(info, &queries)
			.into_meta()
	}
}

impl<S> GraphQLValue<S> for Query
where
	S: ScalarValue,
{
	type Context = ();
	type TypeInfo = Vec<ApiGraphQLType>;

	fn type_name<'i>(&self, info: &'i Self::TypeInfo) -> Option<&'i str> {
		<Self as GraphQLType<S>>::name(info)
	}
}

impl GraphQLValueAsync for Query {
	fn resolve_field_async<'b>(
		&'b self,
		_info: &'b Self::TypeInfo,
		_field_name: &'b str,
		_arguments: &'b Arguments<DefaultScalarValue>,
		_executor: &'b Executor<Self::Context, DefaultScalarValue>,
	) -> BoxFuture<'b, ExecutionResult<DefaultScalarValue>> {


		println!("{} {:?}", _field_name, _arguments);

		todo!()
	}
}

fn build_field_from_property<'r, S>(
	registry: &mut Registry<'r, S>,
	property: &GraphQLProperty,
	scalar_type: &ScalarType,
	enforce_required: bool,
) -> Field<'r, S>
where
	S: ScalarValue,
{
	fn build_field<'r, T, S>(
		registry: &mut Registry<'r, S>,
		property: &GraphQLProperty,
		required: bool,
	) -> Field<'r, S>
	where
		S: ScalarValue + 'r,
		T: GraphQLType<S, Context = (), TypeInfo = ()>,
	{
		let is_array = matches!(property.scalar_type, ScalarType::Array(_));

		if required && !is_array {
			registry.field::<T>(property.name.as_str(), &())
		} else {
			registry.field::<Option<T>>(property.name.as_str(), &())
		}
	}

	match scalar_type {
		ScalarType::Array(t) => {
			let mut field = build_field_from_property(registry, property, &t, false);

			if property.required && enforce_required {
				field.field_type = juniper::Type::NonNullList(Box::new(field.field_type));
			} else {
				field.field_type = juniper::Type::List(Box::new(field.field_type));
			}

			field
		}

		ScalarType::Enum(_) => build_field::<String, S>(registry, property, property.required),
		ScalarType::String => build_field::<String, S>(registry, property, property.required),
		ScalarType::Object => build_field::<String, S>(registry, property, property.required),
		ScalarType::Float => build_field::<f64, S>(registry, property, property.required),
		ScalarType::Int => build_field::<i32, S>(registry, property, property.required),
		ScalarType::Boolean => build_field::<bool, S>(registry, property, property.required),
	}
}

pub struct QueryField;

impl<S> GraphQLType<S> for QueryField
where
	S: ScalarValue,
{
	fn name(info: &Self::TypeInfo) -> Option<&str> {
		Some(info.name.as_str())
	}

	fn meta<'r>(info: &Self::TypeInfo, registry: &mut Registry<'r, S>) -> MetaType<'r, S>
	where
		S: 'r,
	{
		let mut fields = Vec::new();

		for property in &info.properties {
			let field = build_field_from_property(registry, &property, &property.scalar_type, true);

			fields.push(field);
		}

		registry
			.build_object_type::<QueryField>(info, &fields)
			.into_meta()
	}
}

impl<S> GraphQLValue<S> for QueryField
where
	S: ScalarValue,
{
	type Context = ();
	type TypeInfo = ApiGraphQLType;

	fn type_name<'i>(&self, info: &'i Self::TypeInfo) -> Option<&'i str> {
		<Self as GraphQLType<S>>::name(info)
	}
}

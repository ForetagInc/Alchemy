pub mod enums;
pub mod errors;
pub mod fields;
pub mod operations;

use crate::api::schema::fields::SchemaFieldFactory;
use crate::api::schema::operations::{OperationRegistry, OperationType};
use juniper::meta::MetaType;
use juniper::{
	Arguments, BoxFuture, EmptySubscription, ExecutionResult, Executor, GraphQLType,
	GraphQLValue, GraphQLValueAsync, Registry, RootNode, ScalarValue,
};
use std::sync::Arc;

use crate::lib::database::api::*;

pub type Schema = RootNode<'static, Query, Mutation, EmptySubscription>;

pub fn schema(map: DbMap) -> Schema {
	let mut operation_registry = OperationRegistry::new();

	for p in map.primitives {
		match p {
			DbPrimitive::Entity(t) => {
				let mut relationships = Vec::new();

				for relationship in &map.relationships {
					if relationship.from.name == t.name {
						relationships.push(relationship.clone())
					}
				}

				operation_registry.register_entity(t, relationships);
			}
			DbPrimitive::Enum(_) => {}
		}
	}

	let relationships = Arc::new(map.relationships.clone());
	let schema_info = SchemaData {
		operation_registry: Arc::new(operation_registry),
		relationships,
	};

	RootNode::new_with_info(
		Query,
		Mutation,
		EmptySubscription::new(),
		schema_info.clone(),
		schema_info,
		(),
	)
}

#[derive(Clone)]
pub struct SchemaData<S>
where
	S: ScalarValue + Send + Sync,
{
	operation_registry: Arc<OperationRegistry<S>>,
	relationships: Arc<Vec<DbRelationship>>,
}

fn resolve_field_async<'a, S>(
	info: &'a SchemaData<S>,
	field_name: &'a str,
	arguments: &'a Arguments<S>,
	executor: &'a Executor<(), S>,
) -> BoxFuture<'a, ExecutionResult<S>>
where
	S: ScalarValue + Send + Sync,
{
	Box::pin(async move {
		executor
			.resolve_async(
				info,
				&SchemaFieldFactory::new_resolver(field_name, arguments),
			)
			.await
	})
}

pub struct Query;

impl<S> GraphQLType<S> for Query
where
	S: ScalarValue + Send + Sync,
{
	fn name(_: &Self::TypeInfo) -> Option<&str> {
		Some("Query")
	}

	fn meta<'r>(info: &Self::TypeInfo, registry: &mut Registry<'r, S>) -> MetaType<'r, S>
	where
		S: 'r,
	{
		let mut queries = Vec::new();

		for (name, operation) in info.operation_registry.get_operations(OperationType::Query) {
			queries.push(SchemaFieldFactory::new(
				name,
				operation,
				registry,
				&info.operation_registry,
			));
		}

		registry
			.build_object_type::<Query>(info, &queries)
			.into_meta()
	}
}

impl<S> GraphQLValue<S> for Query
where
	S: ScalarValue + Send + Sync,
{
	type Context = ();
	type TypeInfo = SchemaData<S>;

	fn type_name<'i>(&self, info: &'i Self::TypeInfo) -> Option<&'i str> {
		<Self as GraphQLType<S>>::name(info)
	}
}

impl<S> GraphQLValueAsync<S> for Query
where
	S: ScalarValue + Send + Sync,
{
	fn resolve_field_async<'b>(
		&'b self,
		info: &'b Self::TypeInfo,
		field_name: &'b str,
		arguments: &'b Arguments<S>,
		executor: &'b Executor<Self::Context, S>,
	) -> BoxFuture<'b, ExecutionResult<S>> {
		resolve_field_async(info, field_name, arguments, executor)
	}
}

pub struct Mutation;

impl<S> GraphQLType<S> for Mutation
where
	S: ScalarValue + Send + Sync,
{
	fn name(_: &Self::TypeInfo) -> Option<&str> {
		Some("Mutation")
	}

	fn meta<'r>(info: &Self::TypeInfo, registry: &mut Registry<'r, S>) -> MetaType<'r, S>
	where
		S: 'r,
	{
		let mut queries = Vec::new();

		for (name, operation) in info
			.operation_registry
			.get_operations(OperationType::Mutation)
		{
			queries.push(SchemaFieldFactory::new(
				name,
				operation,
				registry,
				&info.operation_registry,
			));
		}

		registry
			.build_object_type::<Mutation>(info, &queries)
			.into_meta()
	}
}

impl<S> GraphQLValue<S> for Mutation
where
	S: ScalarValue + Send + Sync,
{
	type Context = ();
	type TypeInfo = SchemaData<S>;

	fn type_name<'i>(&self, info: &'i Self::TypeInfo) -> Option<&'i str> {
		<Self as GraphQLType<S>>::name(info)
	}
}

impl<S> GraphQLValueAsync<S> for Mutation
where
	S: ScalarValue + Send + Sync,
{
	fn resolve_field_async<'b>(
		&'b self,
		info: &'b Self::TypeInfo,
		field_name: &'b str,
		arguments: &'b Arguments<S>,
		executor: &'b Executor<Self::Context, S>,
	) -> BoxFuture<'b, ExecutionResult<S>> {
		resolve_field_async(info, field_name, arguments, executor)
	}
}

pub mod enums;
pub mod errors;
pub mod fields;
pub mod operations;

use crate::api::schema::fields::QueryFieldFactory;
use crate::api::schema::operations::OperationRegistry;
use juniper::meta::MetaType;
use juniper::{
	Arguments, BoxFuture, EmptyMutation, EmptySubscription, ExecutionResult, Executor, GraphQLType,
	GraphQLValue, GraphQLValueAsync, Registry, RootNode, ScalarValue,
};

use crate::lib::database::api::*;

pub type Schema = RootNode<'static, Query, EmptyMutation, EmptySubscription>;

fn owns_relationship(relationship: &DbRelationship, entity_name: &str) -> bool {
	match relationship.direction {
		DbRelationshipDirection::Inbound => relationship.to.name == entity_name,
		DbRelationshipDirection::Outbound => relationship.from.name == entity_name,
		DbRelationshipDirection::Any => {
			relationship.from.name == entity_name || relationship.to.name == entity_name
		}
	}
}

pub fn schema(map: DbMap) -> Schema {
	let mut operation_registry = OperationRegistry::new();

	for p in map.primitives {
		match p {
			DbPrimitive::Entity(t) => {
				let mut relationships = Vec::new();

				for relationship in &map.relationships {
					if owns_relationship(&relationship, &t.name) {
						relationships.push(relationship.clone())
					}
				}

				operation_registry.register_entity(t, relationships);
			}
			DbPrimitive::Enum(_) => {}
		}
	}

	RootNode::new_with_info(
		Query,
		EmptyMutation::new(),
		EmptySubscription::new(),
		QueryData {
			operation_registry,
			relationships: map.relationships.clone(),
		},
		(),
		(),
	)
}

pub struct QueryData<S>
where
	S: ScalarValue + Send + Sync,
{
	operation_registry: OperationRegistry<S>,
	relationships: Vec<DbRelationship>,
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

		for (name, operation) in info.operation_registry.get_operations() {
			queries.push(QueryFieldFactory::new(
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
	type TypeInfo = QueryData<S>;

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
		Box::pin(async move {
			executor
				.resolve_async(
					info,
					&QueryFieldFactory::new_resolver(field_name, arguments),
				)
				.await
		})
	}
}

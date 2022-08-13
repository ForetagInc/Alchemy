use actix_web::{
	web::{Data, Payload as ActixPayload},
	Error as ActixError, HttpRequest as ActixRequest, HttpResponse as ActixResponse,
};
use std::sync::Mutex;

use juniper_actix::{graphql_handler, playground_handler};

use crate::api::schema::Schema;

pub async fn graphql_api_route(
	req: ActixRequest,
	payload: ActixPayload,
	schema: Data<Mutex<Schema>>,
) -> Result<ActixResponse, ActixError> {
	graphql_handler(&schema.lock().unwrap(), &(), req, payload).await
}

pub async fn playground_api_route() -> Result<ActixResponse, ActixError> {
	playground_handler("/api/graphql", Some("/api/graphql_subscriptions")).await
}

#![feature(trait_alias)]
#![feature(derive_default_enum)]
#![feature(const_try)]

extern crate juniper;

#[macro_use]
extern crate derivative;
#[macro_use]
extern crate juniper_codegen;

use actix_cors::Cors;
use actix_web::{
	http::header,
	middleware,
	web::{self, Data},
	App, HttpServer,
};

mod api;
mod lib;
mod meta;

use lib::database::generate_sdl;
use lib::CONFIG;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	pluralizer::initialize();

	let app_port = CONFIG.app_port.parse::<u16>().unwrap_or(8080);

	println!("Starting Alchemy on port {:?}", app_port);

	let map = generate_sdl().await;
	let api_schema = Data::new(api::schema::schema(map.clone()));

	let meta_schema = Data::new(meta::graphql::schema());

	// Actix server
	HttpServer::new(move || {
		App::new()
			.app_data(meta_schema.clone())
			.app_data(api_schema.clone())
			.wrap(
				Cors::default()
					.allow_any_origin()
					.allowed_methods(vec!["POST", "GET"])
					.allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
					.allowed_header(header::CONTENT_TYPE)
					.supports_credentials()
					.max_age(3600),
			)
			.wrap(middleware::Compress::default())
			.wrap(middleware::Logger::default())
			.service(
				web::resource("/api/graphql")
					.route(web::post().to(api::server::graphql_api_route))
					.route(web::get().to(api::server::graphql_api_route)),
			)
			.service(
				web::resource("/api/playground")
					.route(web::get().to(api::server::playground_api_route)),
			)
			.service(
				web::resource("/meta/graphql")
					.route(web::post().to(meta::graphql::server::graphql_meta_route))
					.route(web::get().to(meta::graphql::server::graphql_meta_route)),
			)
			.service(
				web::resource("/meta/playground")
					.route(web::get().to(meta::graphql::server::playground_meta_route)),
			)
	})
	.bind(("0.0.0.0", app_port))?
	.run()
	.await
}

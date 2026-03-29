mod app_state;
mod extractors;
mod handlers;
mod http_errors;
mod ui_state;

#[cfg(test)]
pub use self::app_state::tests;
pub use self::ui_state::{
    Status, StatusLevel, SubscriptionState, UiPlatformState, UiState, WebhookUrlType,
};

use crate::{
    api::Api,
    config::Config,
    database::Database,
    directories::Directories,
    js_runtime::JsRuntime,
    network::Network,
    scheduler::Scheduler,
    search::{SearchIndex, populate_search_index},
    templates::create_templates,
};
use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, Result, middleware, web};
use anyhow::Context;
pub use app_state::AppState;
use handlers::SecutilsOpenApi;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;
use tracing_actix_web::TracingLogger;
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;

#[tokio::main]
pub async fn run(config: Config, http_port: u16) -> Result<(), anyhow::Error> {
    let datastore_dir = Directories::ensure_data_dir_exists()?;
    info!("Data is available at {}", datastore_dir.as_path().display());
    let search_index = SearchIndex::open_path(datastore_dir.join(format!(
        "search_index_v{}",
        config.components.search_index_version
    )))?;

    let db_url = format!(
        "postgres://{}@{}:{}/{}",
        if let Some(ref password) = config.db.password {
            format!(
                "{}:{}",
                urlencoding::encode(&config.db.username),
                urlencoding::encode(password)
            )
        } else {
            config.db.username.clone()
        },
        config.db.host,
        config.db.port,
        urlencoding::encode(&config.db.name)
    );
    let database = Database::create(
        PgPoolOptions::new()
            .max_connections(config.db.max_connections)
            .min_connections(config.db.min_connections)
            .acquire_timeout(config.db.acquire_timeout)
            .max_lifetime(config.db.max_lifetime)
            .idle_timeout(config.db.idle_timeout)
            .test_before_acquire(true)
            .connect(&db_url)
            .await?,
    )
    .await?;

    let api = Arc::new(Api::new(
        config.clone(),
        database,
        search_index,
        Network::create(&config)?,
        create_templates()?,
    ));

    populate_search_index(&api).await?;

    Scheduler::start(api.clone()).await?;

    JsRuntime::init_platform();

    let max_responder_body_size = config.utils.max_responder_body_size;
    let max_import_file_size = config.platform.max_import_file_size;
    let state = web::Data::new(AppState::new(config, api.clone()));
    let http_server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compat::new(TracingLogger::default()))
            .wrap(middleware::Compat::new(middleware::Compress::default()))
            .wrap(middleware::NormalizePath::trim())
            .app_data(state.clone())
            .service(RapiDoc::with_openapi(
                "/api-docs/openapi.json",
                SecutilsOpenApi::openapi(),
            ))
            .service(handlers::user_tags::user_tags_list)
            .service(handlers::user_tags::user_tags_create)
            .service(handlers::user_tags::user_tags_update)
            .service(handlers::user_tags::user_tags_delete)
            .service(handlers::user_secrets::user_secrets_list)
            .service(handlers::user_secrets::user_secrets_create)
            .service(handlers::user_secrets::user_secrets_update)
            .service(handlers::user_secrets::user_secrets_delete)
            .service(handlers::user_scripts::user_scripts_list)
            .service(handlers::user_scripts::user_scripts_get)
            .service(handlers::user_scripts::user_scripts_create)
            .service(handlers::user_scripts::user_scripts_update)
            .service(handlers::user_scripts::user_scripts_delete)
            .service(
                web::scope("/api")
                    .route("/status", web::get().to(handlers::status_get))
                    .route("/status", web::post().to(handlers::status_set))
                    .route("/search", web::post().to(handlers::search))
                    .route("/send_message", web::post().to(handlers::send_message))
                    .route(
                        "/user/settings",
                        web::post().to(handlers::user_settings_set),
                    )
                    .route("/user/settings", web::get().to(handlers::user_settings_get))
                    .service(
                        web::scope("/user/data")
                            .app_data(
                                web::JsonConfig::default()
                                    .limit(max_import_file_size)
                                    .error_handler(|err, _req| {
                                        let error_message = err.to_string();
                                        actix_web::error::InternalError::from_response(
                                            err,
                                            HttpResponse::BadRequest()
                                                .json(json!({ "message": error_message })),
                                        )
                                        .into()
                                    }),
                            )
                            .route("/_export", web::post().to(handlers::user_data_export))
                            .route(
                                "/_import_preview",
                                web::post().to(handlers::user_data_import_preview),
                            )
                            .route("/_import", web::post().to(handlers::user_data_import)),
                    )
                    .route(
                        "/user/subscription",
                        web::post().to(handlers::security_subscription_update),
                    )
                    .service(
                        web::scope("/webhooks")
                            .route("/retrack", web::post().to(handlers::webhooks_retrack))
                            .service(
                                web::scope("")
                                    .app_data(
                                        web::PayloadConfig::default()
                                            .limit(max_responder_body_size),
                                    )
                                    .route(
                                        "/{user_handle}/{responder_path:.*}",
                                        web::route().to(handlers::webhooks_responders),
                                    )
                                    .route("", web::route().to(handlers::webhooks_responders)),
                            ),
                    )
                    .route(
                        "/users",
                        web::get().to(handlers::security_users_get_by_email),
                    )
                    .service(
                        web::scope("/users")
                            .route("/signup", web::post().to(handlers::security_users_signup))
                            .route("/email", web::post().to(handlers::security_users_email))
                            .route("/remove", web::post().to(handlers::security_users_remove))
                            .route("/self", web::get().to(handlers::security_users_get_self))
                            .route("/{user_id}", web::get().to(handlers::security_users_get)),
                    )
                    .service(web::scope("/scheduler").route(
                        "/parse_schedule",
                        web::post().to(handlers::scheduler_parse_schedule),
                    ))
                    .service(
                        web::scope("/utils")
                            .service(
                                web::resource([
                                    "/{area}/{resource}",
                                    "/{area}/{resource}/{resource_id}",
                                    "/{area}/{resource}/{resource_id}/{resource_operation}",
                                ])
                                .to(handlers::utils_action),
                            )
                            .wrap(Cors::permissive()),
                    )
                    .service(
                        web::scope("/ui")
                            .route("/state", web::get().to(handlers::ui_state_get))
                            .route("/home/summary", web::get().to(handlers::home_summary_get)),
                    ),
            )
    });

    let http_server_url = format!("0.0.0.0:{http_port}");
    let http_server = http_server
        .bind(&http_server_url)
        .with_context(|| format!("Failed to bind to {}.", &http_server_url))?;

    info!(
        "Secutils.dev API server is available at http://{}",
        http_server_url
    );

    http_server
        .run()
        .await
        .with_context(|| "Failed to run Secutils.dev API server.")
}

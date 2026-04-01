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
            // OpenAPI documentation
            .service(
                RapiDoc::with_openapi("/api-docs/openapi.json", SecutilsOpenApi::openapi())
                    .path("/api-docs"),
            )
            // Tags
            .service(handlers::user_tags::user_tags_list)
            .service(handlers::user_tags::user_tags_create)
            .service(handlers::user_tags::user_tags_update)
            .service(handlers::user_tags::user_tags_delete)
            // Secrets
            .service(handlers::user_secrets::user_secrets_list)
            .service(handlers::user_secrets::user_secrets_create)
            .service(handlers::user_secrets::user_secrets_update)
            .service(handlers::user_secrets::user_secrets_delete)
            // Scripts
            .service(handlers::user_scripts::user_scripts_list)
            .service(handlers::user_scripts::user_scripts_get)
            .service(handlers::user_scripts::user_scripts_create)
            .service(handlers::user_scripts::user_scripts_update)
            .service(handlers::user_scripts::user_scripts_delete)
            // Settings
            .service(handlers::user_settings_get::user_settings_get)
            .service(handlers::user_settings_set::user_settings_set)
            // User data (scoped for custom JSON payload size limit)
            .service(
                web::scope("/api/user/data")
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
                    .service(handlers::user_data_export::user_data_export)
                    .service(handlers::user_data_import::user_data_import_preview)
                    .service(handlers::user_data_import::user_data_import),
            )
            // Status
            .service(handlers::status_get::status_get)
            .service(handlers::status_set::status_set)
            // Search
            .service(handlers::search::search)
            // Users
            .service(handlers::security_users_get_self::security_users_get_self)
            .service(handlers::security_users_get_by_email::security_users_get_by_email)
            .service(handlers::security_users_get::security_users_get)
            .service(handlers::security_users_signup::security_users_signup)
            .service(handlers::security_users_email::security_users_email)
            .service(handlers::security_users_remove::security_users_remove)
            .service(handlers::security_subscription_update::security_subscription_update)
            // Scheduler
            .service(handlers::scheduler_parse_schedule::scheduler_parse_schedule)
            // Messages
            .service(handlers::send_message::send_message)
            // Certificate templates
            .service(handlers::certificate_templates::certificate_templates_list)
            .service(handlers::certificate_templates::certificate_templates_get)
            .service(handlers::certificate_templates::certificate_templates_create)
            .service(handlers::certificate_templates::certificate_templates_update)
            .service(handlers::certificate_templates::certificate_templates_delete)
            .service(handlers::certificate_templates::certificate_templates_generate)
            .service(handlers::certificate_templates::certificate_templates_share)
            .service(handlers::certificate_templates::certificate_templates_unshare)
            .service(handlers::certificate_templates::certificates_fetch)
            // Private keys
            .service(handlers::private_keys::private_keys_list)
            .service(handlers::private_keys::private_keys_get)
            .service(handlers::private_keys::private_keys_create)
            .service(handlers::private_keys::private_keys_update)
            .service(handlers::private_keys::private_keys_delete)
            .service(handlers::private_keys::private_keys_export)
            // Webhooks responders
            .service(handlers::responders::responders_list)
            .service(handlers::responders::responders_create)
            .service(handlers::responders::responders_update)
            .service(handlers::responders::responders_delete)
            .service(handlers::responders::responders_get_history)
            .service(handlers::responders::responders_clear_history)
            .service(handlers::responders::responders_get_stats)
            // Content security policies
            .service(handlers::content_security_policies::csp_list)
            .service(handlers::content_security_policies::csp_get)
            .service(handlers::content_security_policies::csp_create)
            .service(handlers::content_security_policies::csp_update)
            .service(handlers::content_security_policies::csp_delete)
            .service(handlers::content_security_policies::csp_serialize)
            .service(handlers::content_security_policies::csp_share)
            .service(handlers::content_security_policies::csp_unshare)
            // Page trackers
            .service(handlers::page_trackers::page_trackers_list)
            .service(handlers::page_trackers::page_trackers_create)
            .service(handlers::page_trackers::page_trackers_update)
            .service(handlers::page_trackers::page_trackers_delete)
            .service(handlers::page_trackers::page_trackers_get_history)
            .service(handlers::page_trackers::page_trackers_clear_history)
            .service(handlers::page_trackers::page_trackers_get_logs)
            .service(handlers::page_trackers::page_trackers_clear_logs)
            .service(handlers::page_trackers::page_trackers_get_logs_summary)
            .service(handlers::page_trackers::page_trackers_debug)
            // API trackers
            .service(handlers::api_trackers::api_trackers_list)
            .service(handlers::api_trackers::api_trackers_create)
            .service(handlers::api_trackers::api_trackers_update)
            .service(handlers::api_trackers::api_trackers_delete)
            .service(handlers::api_trackers::api_trackers_get_history)
            .service(handlers::api_trackers::api_trackers_clear_history)
            .service(handlers::api_trackers::api_trackers_get_logs)
            .service(handlers::api_trackers::api_trackers_clear_logs)
            .service(handlers::api_trackers::api_trackers_get_logs_summary)
            .service(handlers::api_trackers::api_trackers_test)
            .service(handlers::api_trackers::api_trackers_debug)
            // Remaining routes that still use .route() (webhooks, UI)
            .service(
                web::scope("/api")
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

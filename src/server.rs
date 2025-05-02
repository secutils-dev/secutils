mod app_state;
mod extractors;
mod handlers;
mod http_errors;
mod ui_state;

#[cfg(test)]
pub use self::app_state::tests;
use crate::{
    api::Api,
    config::Config,
    database::Database,
    directories::Directories,
    js_runtime::JsRuntime,
    network::{Network, TokioDnsResolver},
    scheduler::Scheduler,
    search::{SearchIndex, populate_search_index},
    templates::create_templates,
};
use actix_cors::Cors;
use actix_web::{App, HttpServer, Result, middleware, web};
use anyhow::Context;
pub use app_state::AppState;
use lettre::{
    AsyncSmtpTransport, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use sqlx::postgres::PgPoolOptions;
use std::{str::FromStr, sync::Arc};
use tracing::info;
use tracing_actix_web::TracingLogger;
pub use ui_state::{Status, StatusLevel, SubscriptionState, UiState, WebhookUrlType};

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
            .max_connections(100)
            .connect(&db_url)
            .await?,
    )
    .await?;

    let email_transport = if let Some(ref smtp_config) = config.smtp {
        if let Some(ref catch_all_config) = smtp_config.catch_all {
            Mailbox::from_str(catch_all_config.recipient.as_str())
                .with_context(|| "Cannot parse SMTP catch-all recipient.")?;
        }

        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_config.address)?
            .credentials(Credentials::new(
                smtp_config.username.clone(),
                smtp_config.password.clone(),
            ))
            .build()
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost()
    };

    let api = Arc::new(Api::new(
        config.clone(),
        database,
        search_index,
        Network::new(TokioDnsResolver::create(), email_transport),
        create_templates()?,
    ));

    populate_search_index(&api).await?;

    Scheduler::start(api.clone()).await?;

    JsRuntime::init_platform();

    let state = web::Data::new(AppState::new(config, api.clone()));
    let http_server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compat::new(TracingLogger::default()))
            .wrap(middleware::Compat::new(middleware::Compress::default()))
            .wrap(middleware::NormalizePath::trim())
            .app_data(state.clone())
            .service(
                web::scope("/api")
                    .route("/status", web::get().to(handlers::status_get))
                    .route("/status", web::post().to(handlers::status_set))
                    .route("/search", web::post().to(handlers::search))
                    .route("/send_message", web::post().to(handlers::send_message))
                    .route("/user/data", web::post().to(handlers::user_data_set))
                    .route("/user/data", web::get().to(handlers::user_data_get))
                    .route(
                        "/user/subscription",
                        web::post().to(handlers::security_subscription_update),
                    )
                    .route(
                        "/webhooks/{user_handle}/{responder_path:.*}",
                        web::route().to(handlers::webhooks_responders),
                    )
                    .route("/webhooks", web::route().to(handlers::webhooks_responders))
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
                        web::scope("/ui").route("/state", web::get().to(handlers::ui_state_get)),
                    ),
            )
    });

    let http_server_url = format!("0.0.0.0:{}", http_port);
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

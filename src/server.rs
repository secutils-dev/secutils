mod app_state;
mod extractors;
mod handlers;
mod http_errors;
mod ui_state;

use crate::{
    api::Api,
    config::Config,
    database::Database,
    directories::Directories,
    js_runtime::JsRuntime,
    network::{Network, TokioDnsResolver},
    scheduler::Scheduler,
    search::{populate_search_index, SearchIndex},
    security::create_webauthn,
    templates::create_templates,
    users::builtin_users_initializer,
};
use actix_cors::Cors;
use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, HttpServer, Result};
use anyhow::Context;
use lettre::{transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor};
use std::sync::Arc;

#[cfg(test)]
pub use self::app_state::tests;

pub use app_state::AppState;
pub use ui_state::{Status, StatusLevel, UiState, WebhookUrlType};

#[tokio::main]
pub async fn run(
    config: Config,
    session_key: [u8; 64],
    secure_cookies: bool,
    builtin_users: Option<String>,
) -> Result<(), anyhow::Error> {
    let datastore_dir = Directories::ensure_data_dir_exists()?;
    log::info!("Data is available at {}", datastore_dir.as_path().display());
    let search_index = SearchIndex::open_path(datastore_dir.join(format!(
        "search_index_v{}",
        config.components.search_index_version
    )))?;
    let database = Database::open_path(datastore_dir).await?;

    let email_transport = if let Some(ref smtp_config) = config.as_ref().smtp {
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
        create_webauthn(&config)?,
        create_templates()?,
    ));

    if let Some(ref builtin_users) = builtin_users {
        builtin_users_initializer(&api, builtin_users)
            .await
            .with_context(|| "Cannot initialize builtin users")?;
    }

    populate_search_index(&api).await?;

    Scheduler::start(api.clone()).await?;

    JsRuntime::init_platform();

    let http_server_url = format!("0.0.0.0:{}", config.http_port);
    let state = web::Data::new(AppState::new(config, api.clone()));
    let http_server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compat::new(middleware::Compress::default()))
            .wrap(middleware::NormalizePath::trim())
            .wrap(IdentityMiddleware::default())
            // The session middleware must be mounted AFTER the identity middleware: `actix-web`
            // invokes middleware in the OPPOSITE order of registration when it receives an incoming
            // request.
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::from(&session_key))
                    .cookie_secure(secure_cookies)
                    .build(),
            )
            .app_data(state.clone())
            .service(
                web::scope("/api")
                    .route("/status", web::get().to(handlers::status_get))
                    .route("/status", web::post().to(handlers::status_set))
                    .route("/search", web::post().to(handlers::search))
                    .route("/send_message", web::post().to(handlers::send_message))
                    .route("/signin", web::post().to(handlers::security_signin))
                    .route("/signout", web::post().to(handlers::security_signout))
                    .route("/signup", web::post().to(handlers::security_signup))
                    .service(
                        web::scope("/activation")
                            .route(
                                "/complete",
                                web::post().to(handlers::security_activation_complete),
                            )
                            .route(
                                "/send_link",
                                web::post().to(handlers::security_activation_send_link),
                            ),
                    )
                    .service(
                        web::scope("/credentials")
                            .route(
                                "/{credentials}",
                                web::delete().to(handlers::security_credentials_remove),
                            )
                            .route(
                                "/send_link",
                                web::post().to(handlers::security_credentials_send_link),
                            )
                            .route(
                                "/password",
                                web::post().to(handlers::security_credentials_update_password),
                            )
                            .route(
                                "/password/reset",
                                web::post().to(handlers::security_credentials_reset_password),
                            )
                            .route(
                                "/passkey/start",
                                web::post().to(handlers::security_credentials_update_passkey_start),
                            )
                            .route(
                                "/passkey/finish",
                                web::post()
                                    .to(handlers::security_credentials_update_passkey_finish),
                            ),
                    )
                    .service(
                        web::scope("/webauthn")
                            .route(
                                "/signup/start",
                                web::post().to(handlers::security_webauthn_signup_start),
                            )
                            .route(
                                "/signup/finish",
                                web::post().to(handlers::security_webauthn_signup_finish),
                            )
                            .route(
                                "/signin/start",
                                web::post().to(handlers::security_webauthn_signin_start),
                            )
                            .route(
                                "/signin/finish",
                                web::post().to(handlers::security_webauthn_signin_finish),
                            ),
                    )
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
                            .route("/remove", web::post().to(handlers::security_users_remove))
                            .route("/{user_id}", web::get().to(handlers::security_users_get)),
                    )
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

    let http_server = http_server
        .bind(&http_server_url)
        .with_context(|| format!("Failed to bind to {}.", &http_server_url))?;

    log::info!(
        "Secutils.dev API server is available at http://{}",
        http_server_url
    );

    http_server
        .run()
        .await
        .with_context(|| "Failed to run Secutils.dev API server.")
}

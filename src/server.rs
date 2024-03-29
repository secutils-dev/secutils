mod app_state;
mod extractors;
mod handlers;
mod http_errors;
mod ui_state;

use crate::{
    api::Api,
    database::Database,
    directories::Directories,
    js_runtime::JsRuntime,
    network::{Network, TokioDnsResolver},
    scheduler::Scheduler,
    search::{populate_search_index, SearchIndex},
    security::create_webauthn,
    templates::create_templates,
};
use actix_cors::Cors;
use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, HttpServer, Result};
use anyhow::{bail, Context};
use bytes::Buf;
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    Tokio1Executor,
};
use sqlx::postgres::PgPoolOptions;
use std::{str::FromStr, sync::Arc};

#[cfg(test)]
pub use self::app_state::tests;

use crate::{
    config::{Config, RawConfig, SESSION_KEY_LENGTH_BYTES},
    users::BuiltinUser,
};
pub use app_state::AppState;
pub use ui_state::{Status, StatusLevel, SubscriptionState, UiState, WebhookUrlType};

#[tokio::main]
pub async fn run(raw_config: RawConfig) -> Result<(), anyhow::Error> {
    let datastore_dir = Directories::ensure_data_dir_exists()?;
    log::info!("Data is available at {}", datastore_dir.as_path().display());
    let search_index = SearchIndex::open_path(datastore_dir.join(format!(
        "search_index_v{}",
        raw_config.components.search_index_version
    )))?;

    let db_url = format!(
        "postgres://{}@{}:{}/{}",
        if let Some(ref password) = raw_config.db.password {
            format!("{}:{password}", raw_config.db.username)
        } else {
            raw_config.db.username.clone()
        },
        raw_config.db.host,
        raw_config.db.port,
        raw_config.db.name
    );
    let database = Database::create(
        PgPoolOptions::new()
            .max_connections(100)
            .connect(&db_url)
            .await?,
    )
    .await?;

    let email_transport = if let Some(ref smtp_config) = raw_config.smtp {
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

    let http_secure_cookies = !raw_config.security.use_insecure_session_cookie;
    let http_port = raw_config.port;

    if raw_config.security.session_key.len() < SESSION_KEY_LENGTH_BYTES {
        bail!("Session key should be at least {SESSION_KEY_LENGTH_BYTES} characters long.");
    }

    let mut http_session_key = [0; SESSION_KEY_LENGTH_BYTES];
    raw_config
        .security
        .session_key
        .as_bytes()
        .copy_to_slice(&mut http_session_key);

    // Extract and parse builtin users from the configuration.
    let builtin_users = raw_config
        .security
        .builtin_users
        .as_ref()
        .and_then(|users| {
            if users.is_empty() {
                None
            } else {
                Some(
                    users
                        .iter()
                        .map(BuiltinUser::try_from)
                        .collect::<anyhow::Result<Vec<_>>>(),
                )
            }
        })
        .transpose()?;

    let config = Config::from(raw_config);
    let api = Arc::new(Api::new(
        config.clone(),
        database,
        search_index,
        Network::new(TokioDnsResolver::create(), email_transport),
        create_webauthn(&config)?,
        create_templates()?,
    ));

    if let Some(builtin_users) = builtin_users {
        let builtin_users_count = builtin_users.len();

        let users = api.users();
        for builtin_user in builtin_users {
            users
                .upsert_builtin(builtin_user)
                .await
                .with_context(|| "Cannot initialize builtin user")?;
        }

        log::info!("Initialized {builtin_users_count} builtin users.");
    }

    populate_search_index(&api).await?;

    Scheduler::start(api.clone()).await?;

    JsRuntime::init_platform();

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
                SessionMiddleware::builder(
                    CookieSessionStore::default(),
                    Key::from(&http_session_key),
                )
                .cookie_secure(http_secure_cookies)
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
                            .route("/self", web::get().to(handlers::security_users_get_self))
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

    let http_server_url = format!("0.0.0.0:{}", http_port);
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

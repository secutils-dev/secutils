mod app_state;
mod extractors;
mod handlers;
mod http_errors;
mod status;

use crate::{
    api::Api,
    config::Config,
    datastore::Datastore,
    directories::Directories,
    network::{Network, TokioDnsResolver},
    scheduler::Scheduler,
    search::search_index_initializer,
    security::{create_webauthn, Security},
    server::app_state::AppState,
    users::builtin_users_initializer,
};
use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, HttpServer, Result};
use anyhow::Context;

#[actix_rt::main]
pub async fn run(
    config: Config,
    session_key: [u8; 64],
    secure_cookies: bool,
    builtin_users: Option<String>,
) -> Result<(), anyhow::Error> {
    let datastore_dir = Directories::ensure_data_dir_exists()?;
    log::info!(
        "Secutils.dev data is available at {}",
        datastore_dir.as_path().display()
    );

    let datastore = Datastore::open(&config, datastore_dir).await?;
    let api = Api::new(config.clone(), datastore.clone());

    if let Some(ref builtin_users) = builtin_users {
        builtin_users_initializer(&api, builtin_users)
            .await
            .with_context(|| "Cannot initialize builtin users")?;
    }

    let security = Security::new(
        config.clone(),
        datastore.primary_db.clone(),
        create_webauthn(&config)?,
    );

    search_index_initializer(&api).await?;

    Scheduler::start(Api::new(config.clone(), datastore.clone())).await?;

    let http_server_url = format!("0.0.0.0:{}", config.http_port);
    let state = web::Data::new(AppState::new(
        config,
        security,
        api,
        Network::new(TokioDnsResolver::create()?),
    ));
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
                    .route("/user", web::get().to(handlers::user_get))
                    .route("/user/data", web::post().to(handlers::user_data_set))
                    .route("/user/data", web::get().to(handlers::user_data_get))
                    .route(
                        "/webhooks/ar/{user_handle}/{name}",
                        web::route().to(handlers::webhooks_auto_responders),
                    )
                    .service(
                        web::scope("/users")
                            .route("/remove", web::post().to(handlers::security_users_remove)),
                    )
                    .service(
                        web::scope("/utils")
                            .route("/action", web::post().to(handlers::utils_handle_action)),
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

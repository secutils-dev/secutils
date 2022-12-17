mod app_state;
mod extractors;
mod handlers;
mod status;

use crate::{
    api::Api, config::Config, datastore::Datastore, file_cache::FileCache,
    server::app_state::AppState, users::initialize_builtin_users,
};
use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, HttpServer, Result};
use anyhow::Context;

#[actix_rt::main]
pub async fn run(
    config: Config,
    session_key: [u8; 64],
    builtin_users: Option<String>,
) -> Result<(), anyhow::Error> {
    let api = Api::new(
        config.clone(),
        FileCache::ensure_cache_dir_exists("indices").and_then(Datastore::open)?,
    );

    if let Some(ref builtin_users) = builtin_users {
        initialize_builtin_users(&api, builtin_users)
            .with_context(|| "Cannot initialize builtin users")?;
    }

    let http_server_url = format!("0.0.0.0:{}", config.http_port);
    let state = web::Data::new(AppState::new(config, api));
    let http_server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compat::new(middleware::Compress::default()))
            .wrap(middleware::NormalizePath::trim())
            .wrap(IdentityMiddleware::default())
            // The session middleware must be mounted AFTER the identity middleware: `actix-web`
            // invokes middleware in the OPPOSITE order of registration when it receives an incoming
            // request.
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                Key::from(&session_key),
            ))
            .app_data(state.clone())
            .service(
                web::scope("/api")
                    .route("/status", web::get().to(handlers::status_get))
                    .route("/status", web::post().to(handlers::status_set))
                    .route("/send_message", web::post().to(handlers::send_message))
                    .route("/login", web::post().to(handlers::security_login))
                    .route("/logout", web::post().to(handlers::security_logout))
                    .route("/user", web::get().to(handlers::user_get))
                    .route("/user/data", web::post().to(handlers::user_data_set))
                    .route(
                        "/webhooks/ar/{user_handle}/{alias}",
                        web::route().to(handlers::webhooks_auto_responders),
                    )
                    .service(
                        web::scope("/users")
                            .route("/signup", web::post().to(handlers::security_users_signup))
                            .route(
                                "/activate",
                                web::post().to(handlers::security_users_activate),
                            )
                            .route("/remove", web::post().to(handlers::security_users_remove)),
                    )
                    .service(
                        web::scope("/utils")
                            .route("/execute", web::post().to(handlers::utils_execute)),
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

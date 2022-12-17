use crate::{
    error::SecutilsError,
    server::{app_state::AppState, status::Status},
    users::User,
    utils::Util,
};
use actix_web::{web, HttpResponse};
use anyhow::anyhow;
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LicenseJSON {
    max_endpoints: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParametersJSON {
    status: Status,
    license: LicenseJSON,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<User>,
    utils: Vec<Util>,
}

pub async fn ui_state_get(
    state: web::Data<AppState>,
    user: Option<User>,
) -> Result<HttpResponse, SecutilsError> {
    Ok(HttpResponse::Ok().json(ParametersJSON {
        status: state
            .status
            .read()
            .map(|status| *status)
            .map_err(|err| anyhow!("Failed to retrieve server status: {:?}.", err))?,
        license: LicenseJSON { max_endpoints: 1 },
        user,
        utils: vec![
            Util {
                id: "home".to_string(),
                name: "Home".to_string(),
                icon: Some("home".to_string()),
                utils: Some(vec![
                    Util {
                        id: "home__getting_started".to_string(),
                        name: "Getting started".to_string(),
                        icon: None,
                        utils: None,
                    },
                    Util {
                        id: "home__whats_new".to_string(),
                        name: "What's new".to_string(),
                        icon: None,
                        utils: None,
                    },
                ]),
            },
            Util {
                id: "webhooks".to_string(),
                name: "Webhooks".to_string(),
                icon: Some("node".to_string()),
                utils: Some(vec![
                    Util {
                        id: "webhooks__responders".to_string(),
                        name: "Responders".to_string(),
                        icon: None,
                        utils: None,
                    },
                    Util {
                        id: "webhooks__triggers".to_string(),
                        name: "Triggers".to_string(),
                        icon: None,
                        utils: None,
                    },
                ]),
            },
            Util {
                id: "certificates".to_string(),
                name: "Digital Certificates".to_string(),
                icon: Some("securityApp".to_string()),
                utils: Some(vec![
                    Util {
                        id: "certificates__root".to_string(),
                        name: "Root certificates".to_string(),
                        icon: None,
                        utils: None,
                    },
                    Util {
                        id: "certificates__leaf".to_string(),
                        name: "Leaf certificates".to_string(),
                        icon: None,
                        utils: None,
                    },
                    Util {
                        id: "certificates__explorer".to_string(),
                        name: "Certificates explorer".to_string(),
                        icon: None,
                        utils: None,
                    },
                ]),
            },
            Util {
                id: "web_security".to_string(),
                name: "Web Security".to_string(),
                icon: Some("globe".to_string()),
                utils: Some(vec![
                    Util {
                        id: "web_security__csp".to_string(),
                        name: "CSP".to_string(),
                        icon: None,
                        utils: Some(vec![
                            Util {
                                id: "web_security__csp__policies".to_string(),
                                name: "Policies".to_string(),
                                icon: None,
                                utils: None,
                            },
                            Util {
                                id: "web_security__csp__explorer".to_string(),
                                name: "Policies explorer".to_string(),
                                icon: None,
                                utils: None,
                            },
                        ]),
                    },
                    Util {
                        id: "web_security__cors".to_string(),
                        name: "CORS".to_string(),
                        icon: None,
                        utils: Some(vec![Util {
                            id: "web_security__cors__explorer".to_string(),
                            name: "Policies explorer".to_string(),
                            icon: None,
                            utils: None,
                        }]),
                    },
                ]),
            },
            Util {
                id: "web_scrapping".to_string(),
                name: "Web Scrapping".to_string(),
                icon: Some("cut".to_string()),
                utils: Some(vec![
                    Util {
                        id: "web_scrapping__screenshots".to_string(),
                        name: "Screenshots".to_string(),
                        icon: None,
                        utils: None,
                    },
                    Util {
                        id: "web_scrapping__resources".to_string(),
                        name: "Resources scrapper".to_string(),
                        icon: None,
                        utils: None,
                    },
                    Util {
                        id: "web_scrapping__tables".to_string(),
                        name: "Tables scrapper".to_string(),
                        icon: None,
                        utils: None,
                    },
                ]),
            },
        ],
    }))
}

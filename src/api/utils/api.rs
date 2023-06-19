use crate::{datastore::PrimaryDb, utils::Util};
use std::borrow::Cow;

pub struct UtilsApi<'a> {
    primary_db: Cow<'a, PrimaryDb>,
}

impl<'a> UtilsApi<'a> {
    /// Creates Utils API.
    pub fn new(primary_db: &'a PrimaryDb) -> Self {
        Self {
            primary_db: Cow::Borrowed(primary_db),
        }
    }

    /// Returns all available utils.
    pub async fn get_all(&self) -> anyhow::Result<Vec<Util>> {
        self.primary_db.get_utils().await
    }
}

#[cfg(test)]
mod tests {
    use crate::{api::UtilsApi, tests::mock_db};
    use insta::assert_debug_snapshot;

    #[actix_rt::test]
    async fn can_get_all_utils() -> anyhow::Result<()> {
        let mock_db = mock_db().await?;
        let api = UtilsApi::new(&mock_db);

        assert_debug_snapshot!(api.get_all().await?, @r###"
        [
            Util {
                id: 1,
                handle: "home",
                name: "Home",
                keywords: Some(
                    "home start docs guides changes",
                ),
                utils: None,
            },
            Util {
                id: 2,
                handle: "webhooks",
                name: "Webhooks",
                keywords: None,
                utils: Some(
                    [
                        Util {
                            id: 3,
                            handle: "webhooks__responders",
                            name: "Responders",
                            keywords: Some(
                                "hooks webhooks responders auto-responders respond http endpoint",
                            ),
                            utils: None,
                        },
                    ],
                ),
            },
            Util {
                id: 4,
                handle: "certificates",
                name: "Digital Certificates",
                keywords: None,
                utils: Some(
                    [
                        Util {
                            id: 5,
                            handle: "certificates__self_signed_certificates",
                            name: "Self-signed certificates",
                            keywords: Some(
                                "digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki",
                            ),
                            utils: None,
                        },
                    ],
                ),
            },
            Util {
                id: 6,
                handle: "web_security",
                name: "Web Security",
                keywords: None,
                utils: Some(
                    [
                        Util {
                            id: 7,
                            handle: "web_security__csp",
                            name: "CSP",
                            keywords: None,
                            utils: Some(
                                [
                                    Util {
                                        id: 8,
                                        handle: "web_security__csp__policies",
                                        name: "Policies",
                                        keywords: Some(
                                            "csp policies content web security",
                                        ),
                                        utils: None,
                                    },
                                ],
                            ),
                        },
                    ],
                ),
            },
            Util {
                id: 9,
                handle: "web_scrapping",
                name: "Web Scrapping",
                keywords: None,
                utils: Some(
                    [
                        Util {
                            id: 10,
                            handle: "web_scrapping__resources",
                            name: "Resources trackers",
                            keywords: Some(
                                "web scrapping crawl spider scrapper resources tracker track javascript css",
                            ),
                            utils: None,
                        },
                    ],
                ),
            },
        ]
        "###);

        Ok(())
    }
}

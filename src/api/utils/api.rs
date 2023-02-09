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
    use crate::{api::UtilsApi, datastore::PrimaryDb};
    use insta::assert_debug_snapshot;

    #[actix_rt::test]
    async fn can_get_all_utils() -> anyhow::Result<()> {
        let mock_db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        let api = UtilsApi::new(&mock_db);

        assert_debug_snapshot!(api.get_all().await?, @r###"
        [
            Util {
                id: 1,
                handle: "home",
                name: "Home",
                keywords: "home start",
                utils: Some(
                    [
                        Util {
                            id: 2,
                            handle: "home__getting_started",
                            name: "Getting started",
                            keywords: "getting started",
                            utils: None,
                        },
                        Util {
                            id: 3,
                            handle: "home__whats_new",
                            name: "What's new",
                            keywords: "news updates what's new",
                            utils: None,
                        },
                    ],
                ),
            },
            Util {
                id: 4,
                handle: "webhooks",
                name: "Webhooks",
                keywords: "webhooks hooks",
                utils: Some(
                    [
                        Util {
                            id: 5,
                            handle: "webhooks__responders",
                            name: "Responders",
                            keywords: "responders auto-responders respond http endpoint",
                            utils: None,
                        },
                    ],
                ),
            },
            Util {
                id: 6,
                handle: "certificates",
                name: "Digital Certificates",
                keywords: "digital certificates x509 X.509 ssl tls openssl public private key encryption pki",
                utils: Some(
                    [
                        Util {
                            id: 7,
                            handle: "certificates__self_signed_certificates",
                            name: "Self-signed certificates",
                            keywords: "digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki",
                            utils: None,
                        },
                    ],
                ),
            },
            Util {
                id: 8,
                handle: "web_security",
                name: "Web Security",
                keywords: "web security",
                utils: Some(
                    [
                        Util {
                            id: 9,
                            handle: "web_security__csp",
                            name: "CSP",
                            keywords: "csp content security policy",
                            utils: Some(
                                [
                                    Util {
                                        id: 10,
                                        handle: "web_security__csp__policies",
                                        name: "Policies",
                                        keywords: "csp policies content security",
                                        utils: None,
                                    },
                                ],
                            ),
                        },
                    ],
                ),
            },
            Util {
                id: 11,
                handle: "web_scrapping",
                name: "Web Scrapping",
                keywords: "scrapping web puppeteer crawl spider",
                utils: Some(
                    [
                        Util {
                            id: 12,
                            handle: "web_scrapping__resources",
                            name: "Resources scrapper",
                            keywords: "web scrapping scrapper resources",
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

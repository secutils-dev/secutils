use crate::{
    api::Api,
    database::Database,
    network::{DnsResolver, EmailTransport},
    utils::Util,
};
use std::borrow::Cow;

pub struct UtilsApiExt<'a> {
    db: Cow<'a, Database>,
}

impl<'a> UtilsApiExt<'a> {
    /// Creates Utils API.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db: Cow::Borrowed(db),
        }
    }

    /// Returns all available utils.
    pub async fn get_all(&self) -> anyhow::Result<Vec<Util>> {
        self.db.get_utils().await
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to retrieve available utils.
    pub fn utils(&self) -> UtilsApiExt {
        UtilsApiExt::new(&self.db)
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsApiExt;
    use crate::tests::mock_db;
    use insta::assert_debug_snapshot;

    #[actix_rt::test]
    async fn can_get_all_utils() -> anyhow::Result<()> {
        let mock_db = mock_db().await?;
        let api = UtilsApiExt::new(&mock_db);

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
                            handle: "certificates__private_keys",
                            name: "Private keys",
                            keywords: Some(
                                "private keys openssl encryption pki rsa dsa ec ecdsa curve ed25519 pkcs8 pkcs12 pem",
                            ),
                            utils: None,
                        },
                        Util {
                            id: 11,
                            handle: "certificates__certificate_templates",
                            name: "Certificate templates",
                            keywords: Some(
                                "digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki templates",
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
                handle: "web_scraping",
                name: "Web Scraping",
                keywords: None,
                utils: Some(
                    [
                        Util {
                            id: 10,
                            handle: "web_scraping__content",
                            name: "Content trackers",
                            keywords: Some(
                                "web scraping crawl spider scraper scrape content tracker track",
                            ),
                            utils: None,
                        },
                        Util {
                            id: 12,
                            handle: "web_scraping__resources",
                            name: "Resources trackers",
                            keywords: Some(
                                "web scraping crawl spider scraper scrape resources tracker track javascript css",
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

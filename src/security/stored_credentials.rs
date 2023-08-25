use anyhow::{anyhow, bail};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use webauthn_rs::prelude::Passkey;

/// Represents possible stored user credentials.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct StoredCredentials {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passkey: Option<Passkey>,
}

impl StoredCredentials {
    pub fn from_passkey(passkey: Passkey) -> Self {
        Self {
            passkey: Some(passkey),
            ..Default::default()
        }
    }

    /// Tries to create `StoredCredentials` with `password_hash` generated from the provider password.
    pub fn try_from_password(password: &str) -> anyhow::Result<Self> {
        if password.is_empty() {
            bail!("Password cannot be empty.");
        }

        Ok(Self {
            password_hash: Some(
                Argon2::default()
                    .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
                    .map(|hash| hash.to_string())
                    .map_err(|err| anyhow!("Failed to generate a password hash: {}", err))?,
            ),
            ..Default::default()
        })
    }

    pub fn is_empty(&self) -> bool {
        self.password_hash.is_none() && self.passkey.is_none()
    }
}

#[cfg(test)]
mod tests {
    use crate::{security::StoredCredentials, tests::webauthn::SERIALIZED_PASSKEY};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let credentials: StoredCredentials = Default::default();
        assert!(credentials.is_empty());
        assert_json_snapshot!(credentials, @"{}");

        let credentials = StoredCredentials::try_from_password("pass")?;
        assert!(!credentials.is_empty());
        insta::with_settings!({ filters => vec![(r"p=.+", "p=[HASH]")]}, {
             assert_json_snapshot!(credentials, @r###"
             {
               "password_hash": "$argon2id$v=19$m=19456,t=2,p=[HASH]
             }"###);
        });

        let credentials =
            StoredCredentials::from_passkey(serde_json::from_str(SERIALIZED_PASSKEY)?);
        assert!(!credentials.is_empty());
        assert_json_snapshot!(credentials, @r###"
        {
          "passkey": {
            "cred": {
              "cred_id": "CVRiuJoJxH66qt-UWSnODqcnrVB4k_PFFHexRPqCroDAnaxn6_1Q01Y8VpYn8A2LcnpUeb6TBpTQaWUc4d1Mfg",
              "cred": {
                "type_": "ES256",
                "key": {
                  "EC_EC2": {
                    "curve": "SECP256R1",
                    "x": "oRqUciz1zfd4bwCn-UaQ-KyfVDRfQHO5QIZl7PTPLDk",
                    "y": "5-fVS4_f1-EpqxAxVdhKJcXBxv1UcGpM0QB-XIR5gV4"
                  }
                }
              },
              "counter": 0,
              "transports": null,
              "user_verified": false,
              "backup_eligible": false,
              "backup_state": false,
              "registration_policy": "preferred",
              "extensions": {
                "cred_protect": "NotRequested",
                "hmac_create_secret": "NotRequested",
                "appid": "NotRequested",
                "cred_props": "Ignored"
              },
              "attestation": {
                "data": "None",
                "metadata": "None"
              },
              "attestation_format": "None"
            }
          }
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let credentials: StoredCredentials = serde_json::from_str(
            r#"
        {
          "password_hash": "$argon2id$v=19$m=4096,t=3,p=[HASH]",
          "passkey": {
            "cred": {
              "cred_id": "CVRiuJoJxH66qt-UWSnODqcnrVB4k_PFFHexRPqCroDAnaxn6_1Q01Y8VpYn8A2LcnpUeb6TBpTQaWUc4d1Mfg",
              "cred": {
                "type_": "ES256",
                "key": {
                  "EC_EC2": {
                    "curve": "SECP256R1",
                    "x": "oRqUciz1zfd4bwCn-UaQ-KyfVDRfQHO5QIZl7PTPLDk",
                    "y": "5-fVS4_f1-EpqxAxVdhKJcXBxv1UcGpM0QB-XIR5gV4"
                  }
                }
              },
              "counter": 0,
              "transports": null,
              "user_verified": false,
              "backup_eligible": false,
              "backup_state": false,
              "registration_policy": "preferred",
              "extensions": {
                "cred_protect": "NotRequested",
                "hmac_create_secret": "NotRequested",
                "appid": "NotRequested",
                "cred_props": "Ignored"
              },
              "attestation": {
                "data": "None",
                "metadata": "None"
              },
              "attestation_format": "None"
            }
          }
        }
        "#,
        )?;
        assert!(!credentials.is_empty());
        assert_eq!(
            credentials.password_hash,
            Some("$argon2id$v=19$m=4096,t=3,p=[HASH]".to_string())
        );
        assert_eq!(
            credentials.passkey.unwrap().cred_id().0,
            vec![
                9, 84, 98, 184, 154, 9, 196, 126, 186, 170, 223, 148, 89, 41, 206, 14, 167, 39,
                173, 80, 120, 147, 243, 197, 20, 119, 177, 68, 250, 130, 174, 128, 192, 157, 172,
                103, 235, 253, 80, 211, 86, 60, 86, 150, 39, 240, 13, 139, 114, 122, 84, 121, 190,
                147, 6, 148, 208, 105, 101, 28, 225, 221, 76, 126
            ]
        );

        Ok(())
    }
}

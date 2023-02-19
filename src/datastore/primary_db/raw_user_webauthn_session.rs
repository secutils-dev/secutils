use crate::users::UserWebAuthnSession;
use anyhow::Context;
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserWebAuthnSession {
    pub email: String,
    pub session_value: Vec<u8>,
    pub timestamp: i64,
}

impl TryFrom<RawUserWebAuthnSession> for UserWebAuthnSession {
    type Error = anyhow::Error;

    fn try_from(raw_user_webauthn_session: RawUserWebAuthnSession) -> Result<Self, Self::Error> {
        let value = serde_json::from_slice(&raw_user_webauthn_session.session_value).with_context(
            || {
                format!(
                    "Cannot deserialize WebAuthn session ({}).",
                    raw_user_webauthn_session.email
                )
            },
        )?;
        Ok(UserWebAuthnSession {
            email: raw_user_webauthn_session.email,
            value,
            timestamp: OffsetDateTime::from_unix_timestamp(raw_user_webauthn_session.timestamp)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::primary_db::raw_user_webauthn_session::RawUserWebAuthnSession,
        tests::webauthn::{SERIALIZED_AUTHENTICATION_STATE, SERIALIZED_REGISTRATION_STATE},
        users::{UserWebAuthnSession, UserWebAuthnSessionValue},
    };
    use insta::assert_debug_snapshot;

    #[test]
    fn can_convert_from_raw_session() -> anyhow::Result<()> {
        let registration_state_value: UserWebAuthnSessionValue = serde_json::from_str(&format!(
            "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
        ))?;
        let raw_session = RawUserWebAuthnSession {
            email: "test@secutils.dev".to_string(),
            session_value: serde_json::to_vec(&registration_state_value)?,
            // January 1, 2000 11:00:00
            timestamp: 946720800,
        };
        assert_debug_snapshot!(UserWebAuthnSession::try_from(raw_session)?,  @r###"
        UserWebAuthnSession {
            email: "test@secutils.dev",
            value: RegistrationState(
                PasskeyRegistration {
                    rs: RegistrationState {
                        policy: Preferred,
                        exclude_credentials: [],
                        challenge: Base64UrlSafeData(
                            [
                                223,
                                161,
                                90,
                                219,
                                14,
                                74,
                                186,
                                255,
                                52,
                                157,
                                60,
                                210,
                                28,
                                75,
                                219,
                                3,
                                154,
                                213,
                                19,
                                100,
                                38,
                                255,
                                29,
                                40,
                                142,
                                55,
                                15,
                                45,
                                135,
                                129,
                                245,
                                18,
                            ],
                        ),
                        credential_algorithms: [
                            ES256,
                            RS256,
                        ],
                        require_resident_key: false,
                        authenticator_attachment: None,
                        extensions: RequestRegistrationExtensions {
                            cred_protect: None,
                            uvm: Some(
                                true,
                            ),
                            cred_props: Some(
                                true,
                            ),
                            min_pin_length: None,
                            hmac_create_secret: None,
                        },
                        experimental_allow_passkeys: true,
                    },
                },
            ),
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        let authentication_state_value: UserWebAuthnSessionValue = serde_json::from_str(&format!(
            "{{\"AuthenticationState\":{SERIALIZED_AUTHENTICATION_STATE}}}"
        ))?;
        let raw_session = RawUserWebAuthnSession {
            email: "test@secutils.dev".to_string(),
            session_value: serde_json::to_vec(&authentication_state_value)?,
            // January 1, 2000 11:00:00
            timestamp: 946720800,
        };
        assert_debug_snapshot!(UserWebAuthnSession::try_from(raw_session)?,  @r###"
        UserWebAuthnSession {
            email: "test@secutils.dev",
            value: AuthenticationState(
                PasskeyAuthentication {
                    ast: AuthenticationState {
                        credentials: [
                            Credential {
                                cred_id: Base64UrlSafeData(
                                    [
                                        125,
                                        175,
                                        116,
                                        208,
                                        221,
                                        218,
                                        57,
                                        52,
                                        87,
                                        208,
                                        100,
                                        225,
                                        145,
                                        169,
                                        29,
                                        142,
                                        98,
                                        199,
                                        176,
                                        151,
                                        81,
                                        77,
                                        251,
                                        205,
                                        48,
                                        204,
                                        78,
                                        129,
                                        15,
                                        161,
                                        79,
                                        46,
                                        150,
                                        243,
                                        238,
                                        245,
                                        195,
                                        156,
                                        232,
                                        148,
                                        147,
                                        3,
                                        62,
                                        174,
                                        123,
                                        138,
                                        134,
                                        39,
                                        212,
                                        227,
                                        55,
                                        126,
                                        72,
                                        185,
                                        118,
                                        35,
                                        180,
                                        246,
                                        70,
                                        50,
                                        32,
                                        28,
                                        208,
                                        58,
                                    ],
                                ),
                                cred: COSEKey {
                                    type_: ES256,
                                    key: EC_EC2(
                                        COSEEC2Key {
                                            curve: SECP256R1,
                                            x: Base64UrlSafeData(
                                                [
                                                    120,
                                                    178,
                                                    56,
                                                    207,
                                                    109,
                                                    99,
                                                    239,
                                                    185,
                                                    6,
                                                    21,
                                                    138,
                                                    91,
                                                    151,
                                                    55,
                                                    31,
                                                    229,
                                                    230,
                                                    169,
                                                    187,
                                                    117,
                                                    159,
                                                    147,
                                                    225,
                                                    246,
                                                    120,
                                                    38,
                                                    206,
                                                    95,
                                                    78,
                                                    196,
                                                    168,
                                                    76,
                                                ],
                                            ),
                                            y: Base64UrlSafeData(
                                                [
                                                    0,
                                                    166,
                                                    37,
                                                    159,
                                                    121,
                                                    130,
                                                    184,
                                                    133,
                                                    238,
                                                    207,
                                                    162,
                                                    44,
                                                    61,
                                                    62,
                                                    169,
                                                    73,
                                                    77,
                                                    234,
                                                    120,
                                                    4,
                                                    36,
                                                    124,
                                                    49,
                                                    14,
                                                    119,
                                                    155,
                                                    85,
                                                    175,
                                                    72,
                                                    126,
                                                    251,
                                                    189,
                                                ],
                                            ),
                                        },
                                    ),
                                },
                                counter: 0,
                                transports: None,
                                user_verified: false,
                                backup_eligible: false,
                                backup_state: false,
                                registration_policy: Preferred,
                                extensions: RegisteredExtensions {
                                    cred_protect: NotRequested,
                                    hmac_create_secret: NotRequested,
                                    appid: NotRequested,
                                    cred_props: Ignored,
                                },
                                attestation: ParsedAttestation {
                                    data: None,
                                    metadata: None,
                                },
                                attestation_format: None,
                            },
                        ],
                        policy: Preferred,
                        challenge: Base64UrlSafeData(
                            [
                                35,
                                96,
                                116,
                                118,
                                12,
                                194,
                                114,
                                12,
                                36,
                                79,
                                43,
                                148,
                                192,
                                14,
                                50,
                                7,
                                33,
                                112,
                                229,
                                176,
                                64,
                                151,
                                77,
                                154,
                                197,
                                193,
                                4,
                                161,
                                3,
                                110,
                                73,
                                83,
                            ],
                        ),
                        appid: None,
                        allow_backup_eligible_upgrade: true,
                    },
                },
            ),
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        Ok(())
    }
}

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PublicKeyAlgorithm {
    Rsa,
    Dsa,
    Ecdsa,
    Ed25519,
}

#[cfg(test)]
mod tests {
    use crate::utils::PublicKeyAlgorithm;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(PublicKeyAlgorithm::Rsa, @r###""rsa""###);
        assert_json_snapshot!(PublicKeyAlgorithm::Dsa, @r###""dsa""###);
        assert_json_snapshot!(PublicKeyAlgorithm::Ecdsa, @r###""ecdsa""###);
        assert_json_snapshot!(PublicKeyAlgorithm::Ed25519, @r###""ed25519""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PublicKeyAlgorithm>(r###""rsa""###)?,
            PublicKeyAlgorithm::Rsa
        );
        assert_eq!(
            serde_json::from_str::<PublicKeyAlgorithm>(r###""dsa""###)?,
            PublicKeyAlgorithm::Dsa
        );
        assert_eq!(
            serde_json::from_str::<PublicKeyAlgorithm>(r###""ecdsa""###)?,
            PublicKeyAlgorithm::Ecdsa
        );
        assert_eq!(
            serde_json::from_str::<PublicKeyAlgorithm>(r###""ed25519""###)?,
            PublicKeyAlgorithm::Ed25519
        );

        Ok(())
    }
}

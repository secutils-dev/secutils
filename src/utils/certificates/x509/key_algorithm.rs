use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum KeyAlgorithm {
    Rsa,
    Dsa,
    Ecdsa,
    Ed25519,
}

#[cfg(test)]
mod tests {
    use crate::utils::KeyAlgorithm;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(KeyAlgorithm::Rsa, @r###""rsa""###);
        assert_json_snapshot!(KeyAlgorithm::Dsa, @r###""dsa""###);
        assert_json_snapshot!(KeyAlgorithm::Ecdsa, @r###""ecdsa""###);
        assert_json_snapshot!(KeyAlgorithm::Ed25519, @r###""ed25519""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r###""rsa""###)?,
            KeyAlgorithm::Rsa
        );
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r###""dsa""###)?,
            KeyAlgorithm::Dsa
        );
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r###""ecdsa""###)?,
            KeyAlgorithm::Ecdsa
        );
        assert_eq!(
            serde_json::from_str::<KeyAlgorithm>(r###""ed25519""###)?,
            KeyAlgorithm::Ed25519
        );

        Ok(())
    }
}

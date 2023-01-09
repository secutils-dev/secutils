use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SignatureAlgorithm {
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
    Ed25519,
}

#[cfg(test)]
mod tests {
    use crate::utils::SignatureAlgorithm;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(SignatureAlgorithm::Md5, @r###""md5""###);
        assert_json_snapshot!(SignatureAlgorithm::Sha1, @r###""sha1""###);
        assert_json_snapshot!(SignatureAlgorithm::Sha256, @r###""sha256""###);
        assert_json_snapshot!(SignatureAlgorithm::Sha384, @r###""sha384""###);
        assert_json_snapshot!(SignatureAlgorithm::Sha512, @r###""sha512""###);
        assert_json_snapshot!(SignatureAlgorithm::Ed25519, @r###""ed25519""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<SignatureAlgorithm>(r###""md5""###)?,
            SignatureAlgorithm::Md5
        );
        assert_eq!(
            serde_json::from_str::<SignatureAlgorithm>(r###""sha1""###)?,
            SignatureAlgorithm::Sha1
        );
        assert_eq!(
            serde_json::from_str::<SignatureAlgorithm>(r###""sha256""###)?,
            SignatureAlgorithm::Sha256
        );
        assert_eq!(
            serde_json::from_str::<SignatureAlgorithm>(r###""sha384""###)?,
            SignatureAlgorithm::Sha384
        );
        assert_eq!(
            serde_json::from_str::<SignatureAlgorithm>(r###""sha512""###)?,
            SignatureAlgorithm::Sha512
        );
        assert_eq!(
            serde_json::from_str::<SignatureAlgorithm>(r###""ed25519""###)?,
            SignatureAlgorithm::Ed25519
        );

        Ok(())
    }
}

use serde::{Deserialize, Serialize};

/// Defines named elliptic curves used with Elliptic Curve Digital Signature Algorithm (ECDSA).
/// See https://www.rfc-editor.org/rfc/rfc8422.html#appendix-A.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EllipticCurve {
    /// Elliptic curve prime256v1 (ANSI X9.62) / secp256r1 (SECG) / NIST P-256 (NIST).
    SECP256R1 = 415,
    /// Elliptic curve secp384r1 (SECG) / NIST P-384 (NIST).
    SECP384R1 = 715,
    /// Elliptic curve secp521r1 (SECG) / NIST P-521 (NIST).
    SECP521R1 = 716,
}

#[cfg(test)]
mod tests {
    use crate::utils::EllipticCurve;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() {
        assert_json_snapshot!(EllipticCurve::SECP256R1, @r###""secp256r1""###);
        assert_json_snapshot!(EllipticCurve::SECP384R1, @r###""secp384r1""###);
        assert_json_snapshot!(EllipticCurve::SECP521R1, @r###""secp521r1""###);
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<EllipticCurve>(r#""secp256r1""#)?,
            EllipticCurve::SECP256R1
        );

        assert_eq!(
            serde_json::from_str::<EllipticCurve>(r#""secp384r1""#)?,
            EllipticCurve::SECP384R1
        );

        assert_eq!(
            serde_json::from_str::<EllipticCurve>(r#""secp521r1""#)?,
            EllipticCurve::SECP521R1
        );

        Ok(())
    }

    #[test]
    fn as_number() {
        assert_eq!(EllipticCurve::SECP256R1 as u32, 415);
        assert_eq!(EllipticCurve::SECP384R1 as u32, 715);
        assert_eq!(EllipticCurve::SECP521R1 as u32, 716);
    }
}

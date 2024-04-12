use std::str::FromStr;

/// Kratos email template type, see https://www.ory.sh/docs/reference/api#tag/courier/operation/getCourierMessage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailTemplateType {
    RecoveryCode,
    VerificationCode,
}

impl FromStr for EmailTemplateType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recovery_code_valid" => Ok(Self::RecoveryCode),
            "verification_code_valid" => Ok(Self::VerificationCode),
            _ => Err(anyhow::anyhow!("Unknown email template type: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::security::kratos::EmailTemplateType;
    use std::str::FromStr;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            EmailTemplateType::RecoveryCode,
            EmailTemplateType::from_str("recovery_code_valid")?
        );

        assert_eq!(
            EmailTemplateType::VerificationCode,
            EmailTemplateType::from_str("verification_code_valid")?
        );

        assert!(EmailTemplateType::from_str("verification_code_invalid").is_err());

        Ok(())
    }
}

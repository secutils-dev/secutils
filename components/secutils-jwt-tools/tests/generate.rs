use jsonwebtoken::{DecodingKey, Validation, decode};
use serde_json::Value;
use std::process::Command;
use time::OffsetDateTime;

#[test]
fn generates_signed_token() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_secutils-jwt-tools"))
        .args([
            "generate",
            "--secret",
            "test-jwt-secret",
            "--sub",
            "dev@secutils.dev",
            "--exp",
            "1h",
        ])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let token = String::from_utf8(output.stdout)?;
    let claims = decode::<Value>(
        token.trim(),
        &DecodingKey::from_secret(b"test-jwt-secret")?,
        &Validation::default(),
    )?
    .claims;
    assert_eq!(claims["sub"], "dev@secutils.dev");
    assert!(claims["exp"].as_i64().unwrap() > OffsetDateTime::now_utc().unix_timestamp());
    Ok(())
}

#[test]
fn rejects_empty_signing_secret() -> anyhow::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_secutils-jwt-tools"))
        .args([
            "generate",
            "--secret",
            "",
            "--sub",
            "dev@secutils.dev",
            "--exp",
            "1h",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("InvalidKeyFormat"));
    Ok(())
}

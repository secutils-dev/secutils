use serde_derive::{Deserialize, Serialize};

/// Default max import file size: 10 MB.
const fn default_max_import_file_size() -> usize {
    10 * 1024 * 1024
}

/// Platform-level configuration for limits and settings exposed to the UI.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct PlatformConfig {
    /// Maximum allowed size (in bytes) for user data import files.
    #[serde(default = "default_max_import_file_size")]
    pub max_import_file_size: usize,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            max_import_file_size: default_max_import_file_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlatformConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(PlatformConfig::default(), @r###"
        max_import_file_size = 10485760
        "###);

        let config = PlatformConfig {
            max_import_file_size: 20 * 1024 * 1024,
        };
        assert_toml_snapshot!(config, @r###"
        max_import_file_size = 20971520
        "###);
    }

    #[test]
    fn deserialization() {
        let config: PlatformConfig = toml::from_str("").unwrap();
        assert_eq!(config, PlatformConfig::default());

        let config: PlatformConfig = toml::from_str("max_import_file_size = 5242880").unwrap();
        assert_eq!(
            config,
            PlatformConfig {
                max_import_file_size: 5 * 1024 * 1024,
            }
        );
    }
}

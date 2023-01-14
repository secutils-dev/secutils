use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

struct KnownUserSettingDescriptor {
    setting_key: &'static str,
    setting_value_validator: fn(&serde_json::Value) -> bool,
}

const KNOWN_USER_SETTINGS: [KnownUserSettingDescriptor; 1] = [KnownUserSettingDescriptor {
    setting_key: "certificates.doNotShowSelfSignedWarning",
    setting_value_validator: |value| value.is_boolean(),
}];

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct UserSettings(BTreeMap<String, serde_json::Value>);

#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct UserSettingsSetter(pub BTreeMap<String, Option<serde_json::Value>>);
impl UserSettingsSetter {
    pub fn is_valid(&self) -> bool {
        for (setting_key, setting_value) in self.0.iter() {
            let setting_validator =
                KNOWN_USER_SETTINGS
                    .iter()
                    .find_map(|known_setting_descriptor| {
                        if known_setting_descriptor.setting_key == setting_key {
                            Some(known_setting_descriptor.setting_value_validator)
                        } else {
                            None
                        }
                    });

            // If we cannot find setting value validator then the setting is unknown.
            let setting_validator = if let Some(setting_validator) = setting_validator {
                setting_validator
            } else {
                return false;
            };

            let is_setting_value_valid = setting_value
                .as_ref()
                .map(setting_validator)
                .unwrap_or(true);
            if !is_setting_value_valid {
                return false;
            }
        }
        true
    }

    pub fn into_inner(self) -> BTreeMap<String, Option<serde_json::Value>> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserSettingsSetter;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn should_properly_validate_values() {
        let user_settings = UserSettingsSetter(
            [("certificates.doNotShowSelfSignedWarning".to_string(), None)]
                .into_iter()
                .collect(),
        );
        assert!(user_settings.is_valid());

        let user_settings = UserSettingsSetter(
            [(
                "certificates.doNotShowSelfSignedWarning".to_string(),
                Some(json!(true)),
            )]
            .into_iter()
            .collect(),
        );
        assert!(user_settings.is_valid());

        let user_settings = UserSettingsSetter(
            [
                (
                    "certificates.doNotShowSelfSignedWarning".to_string(),
                    Some(json!(true)),
                ),
                ("unknownSetting".to_string(), None),
            ]
            .into_iter()
            .collect(),
        );
        assert!(!user_settings.is_valid());

        let user_settings =
            UserSettingsSetter([("unknownSetting".to_string(), None)].into_iter().collect());
        assert!(!user_settings.is_valid());

        let user_settings = UserSettingsSetter(
            [("unknownSetting".to_string(), Some(json!(true)))]
                .into_iter()
                .collect(),
        );
        assert!(!user_settings.is_valid());
    }

    #[test]
    fn should_properly_return_inner_value() {
        let user_settings_inner = [(
            "certificates.doNotShowSelfSignedWarning".to_string(),
            Some(json!(true)),
        )]
        .into_iter()
        .collect::<BTreeMap<String, Option<serde_json::Value>>>();

        assert_eq!(
            UserSettingsSetter(user_settings_inner.clone()).into_inner(),
            user_settings_inner
        );
    }
}
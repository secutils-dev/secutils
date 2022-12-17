use serde_derive::Serialize;

#[derive(Serialize)]
pub struct Util {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utils: Option<Vec<Util>>,
}

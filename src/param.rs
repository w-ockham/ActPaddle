use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct KeyerParam {
    #[serde(default)]
    pub wpm: Option<u32>,
    #[serde(default)]
    pub ratio: Option<u32>,
    #[serde(default)]
    pub word_space: Option<u32>,
    #[serde(default)]
    pub letter_space: Option<u32>,
    #[serde(default)]
    pub to_paddle: Option<String>,
    #[serde(default)]
    pub to_straight: Option<String>,
    #[serde(default)]
    pub reverse: Option<bool>,
    #[serde(default)]
    pub ssid: Option<String>,
    #[serde(default)]
    pub del_ssid: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub ssidlist: Option<Vec<String>>,
    #[serde(default)]
    pub init : Option<bool>,
}

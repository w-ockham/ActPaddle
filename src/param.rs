use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug)]
pub struct KeyerParam {
    pub wpm: Option<u32>,
    pub ratio: Option<u32>,
    pub word_space: Option<u32>,
    pub letter_space: Option<u32>,
    pub to_paddle: Option<String>,
    pub to_straight: Option<String>,
    pub reverse: Option<bool>
}
use serde::{Deserialize, Serialize};



#[derive(Serialize, Deserialize, Debug)]
pub struct CustomMessage {
    pub a_field : String,
    pub b_field : u128,
}
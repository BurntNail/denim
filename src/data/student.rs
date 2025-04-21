use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Student {
    pub id: i32,
    pub first_name: String,
    pub preferred_name: Option<String>,
    pub last_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct FormStudent {
    pub first_name: String,
    pub preferred_name: Option<String>,
    pub last_name: String,
}
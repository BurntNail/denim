use serde::Deserialize;

pub mod student;

#[derive(Deserialize)]
pub struct IdForm {
    pub id: i32
}
use chrono::NaiveDateTime;
use maud::Render;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub pref_name: Option<String>,
    pub surname: String,
    pub email: String,
    pub bcrypt_hashed_password: Option<String>,
    pub magic_first_login_characters: Option<String>,
}

impl Render for User {
    fn render_to(&self, buffer: &mut String) {
        match self.pref_name.as_deref() {
            Some(pn) => buffer.push_str(pn),
            None => buffer.push_str(&self.first_name)
        };
        buffer.push(' ');
        buffer.push_str(&self.surname);
    }
}

#[derive(Debug)]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    pub date: NaiveDateTime,
    pub location: Option<String>,
    pub extra_info: Option<String>,
    pub associated_staff_member: Option<Uuid>
}

#[derive(Deserialize)]
pub struct IdForm {
    pub id: Uuid
}
use macros::Entity;
use serde::{Deserialize, Serialize};

#[derive(Entity)]
pub struct User {
    pub username: String,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct NewUser {
    pub username: String,
}

impl From<NewUser> for User {
    fn from(value: NewUser) -> User {
        Self {
            username: value.username,
        }
    }
}

impl User {
    #[allow(dead_code)]
    pub fn new(username: String) -> Self {
        Self { username }
    }
}

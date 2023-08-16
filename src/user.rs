use macros::Entity;
use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;

#[derive(Entity)]
pub struct User {
    pub username: String,
    pub identity_key: PublicKey,
    pub signed_prekey: PublicKey,
    pub prekey_signature: Vec<u8>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct NewUser {
    pub username: String,
    pub identity_key: PublicKey,
    pub signed_prekey: PublicKey,
    pub prekey_signature: Vec<u8>,
}

impl From<NewUser> for User {
    fn from(value: NewUser) -> User {
        Self {
            username: value.username,
            identity_key: value.identity_key,
            prekey_signature: value.prekey_signature,
            signed_prekey: value.signed_prekey,
        }
    }
}

use macros::Entity;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::crypto::PublicKey;

#[derive(Entity)]
pub struct User {
    pub username: String,
    pub identity_key: PublicKey,
    pub signed_prekey: PublicKey,
    pub prekey_signature: Vec<u8>,
    one_time_prekeys: String,
}

impl User {
    pub fn one_time_prekeys(&self) -> Vec<PublicKey> {
        serde_json::from_str(&self.one_time_prekeys).unwrap()
    }

    pub fn insert_prekeys(&mut self, keys: Vec<PublicKey>) {
        let mut stored = serde_json::from_str::<Vec<PublicKey>>(&self.one_time_prekeys).unwrap();
        keys.into_iter().for_each(|k| stored.push(k));
        self.one_time_prekeys = serde_json::to_string(&stored).unwrap();
    }

    pub fn pop_prekey(&mut self) -> Option<PublicKey> {
        let mut stored = serde_json::from_str::<Vec<PublicKey>>(&self.one_time_prekeys).unwrap();
        let result = stored.pop();
        self.one_time_prekeys = serde_json::to_string(&stored).unwrap();
        result
    }
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
            one_time_prekeys: json!(Vec::<PublicKey>::new()).to_string(),
        }
    }
}

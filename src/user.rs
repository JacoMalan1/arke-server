use libsignal_protocol::PublicKey;

pub struct User {
    pub identity_key: PublicKey,
    pub signed_prekey: PublicKey,
    pub prekey_sig: Vec<u8>,
    pub one_time_prekeys: Vec<PublicKey>,
}

impl User {
    #[allow(dead_code)]
    pub fn new(
        identity_key: PublicKey,
        signed_prekey: PublicKey,
        prekey_sig: Vec<u8>,
        one_time_prekeys: Vec<PublicKey>,
    ) -> Self {
        Self {
            identity_key,
            signed_prekey,
            prekey_sig,
            one_time_prekeys,
        }
    }
}

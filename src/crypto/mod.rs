use openssl::{
    ec::EcKey,
    pkey::{PKey, Private, Public},
};
use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Type, Default, Debug, Clone, Serialize, Deserialize)]
#[sqlx(transparent)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    pub fn ec_key(&self) -> Result<EcKey<Public>, openssl::error::ErrorStack> {
        EcKey::public_key_from_pem(&self.0)
    }
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct PrivateKey {
    data: Vec<u8>,
}

impl PrivateKey {
    pub async fn generate() -> Result<(PrivateKey, PublicKey), openssl::error::ErrorStack> {
        let ec_key = tokio::task::spawn_blocking(|| PKey::generate_ed25519())
            .await
            .expect("Couldn't join key generation thread")?
            .ec_key()?;

        let private = PrivateKey {
            data: Vec::from(ec_key.private_key_to_pem()?),
        };
        let public = PublicKey(Vec::from(ec_key.public_key_to_pem()?));

        Ok((private, public))
    }
}

impl Into<EcKey<Private>> for PrivateKey {
    fn into(self) -> EcKey<Private> {
        EcKey::private_key_from_pem(&self.data).unwrap()
    }
}

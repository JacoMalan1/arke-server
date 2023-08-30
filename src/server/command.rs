use crate::user::NewUser;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkeHello {
    pub version: (u8, u8, u8),
}

impl Default for ArkeHello {
    fn default() -> Self {
        let mut version = env!("CARGO_PKG_VERSION")
            .split(".")
            .map(|s| s.parse::<u8>().unwrap());

        let version = (
            version.next().unwrap(),
            version.next().unwrap(),
            version.next().unwrap(),
        );

        Self { version }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "type", content = "payload")]
#[repr(u8)]
pub enum ArkeCommand {
    Hello(ArkeHello) = 0,
    CreateUser(NewUser) = 1,
    Success = 2,
    Goodbye(Option<CommandError>) = 3,
    Error(CommandError) = 4,
    InsertPrekeys(Vec<crate::crypto::PublicKey>) = 5,
}

impl ArkeCommand {
    pub fn discriminant(&self) -> u8 {
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CommandError {
    ServerError { msg: String },
    InvalidSignature { msg: String },
    InvalidKey,
}

impl Into<ArkeCommand> for CommandError {
    fn into(self) -> ArkeCommand {
        ArkeCommand::Goodbye(Some(self))
    }
}

#[async_trait]
pub trait CommandHandler: Send {
    async fn handle(&mut self, command: ArkeCommand) -> ArkeCommand;
}

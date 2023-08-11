use crate::user::NewUser;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "type", content = "payload")]
#[repr(u8)]
pub enum ArkeCommand {
    Hello(String) = 0,
    CreateUser(NewUser),
    Success,
    Goodbye(Option<CommandError>),
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

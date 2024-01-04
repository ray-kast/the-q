use serenity::model::{
    application::{Command, CommandType},
    id::{ApplicationId, CommandId, CommandVersionId, GuildId},
};

use super::{CommandInfo, Data, Trie, TryFromError};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in super::super) struct RegisteredCommand {
    pub(in super::super) id: CommandId,
    pub(in super::super) app: ApplicationId,
    pub(in super::super) guild: Option<GuildId>,
    pub(in super::super) version: CommandVersionId,
    pub(in super::super) info: CommandInfo,
}

impl TryFrom<Command> for RegisteredCommand {
    type Error = TryFromError;

    fn try_from(cmd: Command) -> Result<Self, Self::Error> {
        let Command {
            id,
            kind,
            application_id,
            guild_id,
            name,
            description,
            options,
            dm_permission,
            version,
            ..
        } = cmd;

        let data = match kind {
            CommandType::ChatInput => Data::Slash {
                desc: description,
                trie: Trie::try_build(options)?,
            },
            CommandType::User => Data::User,
            CommandType::Message => Data::Message,
            _ => return Err(TryFromError("Unknown command type")),
        };

        Ok(Self {
            id,
            app: application_id,
            guild: guild_id,
            version,
            info: CommandInfo {
                name,
                data,
                can_dm: dm_permission.unwrap_or(true),
            },
        })
    }
}

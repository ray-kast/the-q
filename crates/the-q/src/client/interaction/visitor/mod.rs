mod command;

mod private {
    use serenity::model::{
        application::interaction::{application_command, autocomplete, message_component, modal},
        guild, id, user,
    };

    pub trait Interaction {
        type Data;

        fn data(&self) -> &Self::Data;

        fn guild_id(&self) -> &Option<id::GuildId>;

        fn member(&self) -> &Option<guild::Member>;

        fn user(&self) -> &user::User;
    }

    impl Interaction for application_command::ApplicationCommandInteraction {
        type Data = application_command::CommandData;

        #[inline]
        fn data(&self) -> &Self::Data { &self.data }

        #[inline]
        fn guild_id(&self) -> &Option<id::GuildId> { &self.guild_id }

        #[inline]
        fn member(&self) -> &Option<guild::Member> { &self.member }

        #[inline]
        fn user(&self) -> &user::User { &self.user }
    }

    impl Interaction for message_component::MessageComponentInteraction {
        type Data = message_component::MessageComponentInteractionData;

        #[inline]
        fn data(&self) -> &Self::Data { &self.data }

        #[inline]
        fn guild_id(&self) -> &Option<id::GuildId> { &self.guild_id }

        #[inline]
        fn member(&self) -> &Option<guild::Member> { &self.member }

        #[inline]
        fn user(&self) -> &user::User { &self.user }
    }

    impl Interaction for autocomplete::AutocompleteInteraction {
        type Data = application_command::CommandData;

        #[inline]
        fn data(&self) -> &Self::Data { &self.data }

        #[inline]
        fn guild_id(&self) -> &Option<id::GuildId> { &self.guild_id }

        #[inline]
        fn member(&self) -> &Option<guild::Member> { &self.member }

        #[inline]
        fn user(&self) -> &user::User { &self.user }
    }

    impl Interaction for modal::ModalSubmitInteraction {
        type Data = modal::ModalSubmitInteractionData;

        #[inline]
        fn data(&self) -> &Self::Data { &self.data }

        #[inline]
        fn guild_id(&self) -> &Option<id::GuildId> { &self.guild_id }

        #[inline]
        fn member(&self) -> &Option<guild::Member> { &self.member }

        #[inline]
        fn user(&self) -> &user::User { &self.user }
    }
}

pub use command::CommandVisitor;
use serenity::model::{
    application::command::CommandOptionType, guild::Member, id::GuildId, user::User,
};

use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Received data was invalid: {0}")]
    Malformed(&'static str),

    // Top-level command type errors
    #[error("Attempted to read options for a non-slash command")]
    NotChatInput,
    #[error("Attempted to read target user for a non-user command")]
    NotUser,
    #[error("Attempted to read target message for a non-message command")]
    NotMessage,

    // Subcommand visitor errors
    #[error("Attempt to read subcommand with none present")]
    MissingSubcommand,
    #[error("Tried to read arguments for subcommand {0:?}")]
    UnhandledSubcommand(Vec<String>),

    // Option visitor errors
    #[error("Required command option {0:?} missing or already visited")]
    MissingOption(String),
    #[error("Command option type mismatch - expected {1}, found {2:?}")]
    BadOptionType(String, &'static str, CommandOptionType),
    #[error("Type mismatch in value of command option {0:?} - expected {1}, found {2:?}")]
    BadOptionValueType(String, &'static str, command::OptionValueType),
    #[error("Error parsing value for {0:?}: {1}")]
    OptionParse(String, Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Trailing arguments: {0:?}")]
    Trailing(Vec<String>),

    // Guild visitor errors
    #[error("Guild-only command run inside DM")]
    GuildRequired,
    #[error("DM-only command run inside guild")]
    DmRequired,
}

trait Describe {
    type Desc: fmt::Debug;

    fn describe(&self) -> Self::Desc;
}

type Result<T> = std::result::Result<T, Error>;

pub struct BasicVisitor<'a, I> {
    int: &'a I,
}

impl<'a, I> BasicVisitor<'a, I> {
    // TODO: make this private
    pub fn new(int: &'a I) -> Self { Self { int } }
}

impl<'a, I: private::Interaction> BasicVisitor<'a, I> {
    #[inline]
    pub fn guild(&self) -> GuildVisitor {
        assert_eq!(self.int.guild_id().is_some(), self.int.member().is_some());
        GuildVisitor(self.int.guild_id().zip(self.int.member().as_ref()))
    }

    #[inline]
    pub fn user(&self) -> &'a User { self.int.user() }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct GuildVisitor<'a>(Option<(GuildId, &'a Member)>);

// TODO: handle the dm_permission field
impl<'a> GuildVisitor<'a> {
    #[inline]
    pub fn optional(self) -> Option<(GuildId, &'a Member)> { self.0 }

    #[inline]
    pub fn required(self) -> Result<(GuildId, &'a Member)> { self.0.ok_or(Error::GuildRequired) }

    #[inline]
    pub fn require_dm(self) -> Result<()> {
        self.0.is_none().then_some(()).ok_or(Error::DmRequired)
    }
}

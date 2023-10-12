//! Types for extracting data from interaction invocations in a type-safe manner

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

        fn id(&self) -> &id::InteractionId;
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

        #[inline]
        fn id(&self) -> &id::InteractionId { &self.id }
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

        #[inline]
        fn id(&self) -> &id::InteractionId { &self.id }
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

        #[inline]
        fn id(&self) -> &id::InteractionId { &self.id }
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

        #[inline]
        fn id(&self) -> &id::InteractionId { &self.id }
    }
}

use std::fmt;

pub use command::CommandVisitor;
use serenity::model::{
    application::command::CommandOptionType,
    guild::Member,
    id::{GuildId, InteractionId},
    user::User,
};

/// An error caused by performing an invalid extraction
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The input data does not conform to the relevant specification
    #[error("Received data was invalid: {0}")]
    Malformed(&'static str),

    // Top-level command type errors
    /// A chat input data extractor was used on a non-chat-input command
    #[error("Attempted to read options for a non-slash command")]
    NotChatInput,
    /// A user target extractor was used on a non-user-context-menu command
    #[error("Attempted to read target user for a non-user command")]
    NotUser,
    /// A message target extractor was used on a non-message-context-menu
    /// command
    #[error("Attempted to read target message for a non-message command")]
    NotMessage,

    // Subcommand visitor errors
    /// The subcommand extractor was used on a command with no subcommands
    #[error("Attempt to read subcommand with none present")]
    MissingSubcommand,
    /// An argument extractor was used on a subcommand invocation without
    /// applying the subcommand extractor first
    #[error("Tried to read arguments for subcommand {0:?}")]
    UnhandledSubcommand(Vec<String>),

    // Option visitor errors
    /// An argument was required but not present in the input
    #[error("Required command option {0:?} missing or already visited")]
    MissingOption(String),
    /// An argument was present in the input but not declared to be of the
    /// correct type
    #[error("Command option type mismatch - expected {1}, found {2:?}")]
    BadOptionType(String, &'static str, CommandOptionType),
    /// An argument was present in the input but its value was not of the
    /// correct type
    #[error("Type mismatch in value of command option {0:?} - expected {1}, found {2:?}")]
    BadOptionValueType(String, &'static str, command::OptionValueType),
    /// A trailing argument was left in the visitor after the handler completed
    #[error("Trailing arguments: {0:?}")]
    Trailing(Vec<String>),

    // Guild visitor errors
    /// The guild ID extractor was used on an interaction invoked outside of a
    /// guild
    #[error("Guild-only interaction run inside DM")]
    GuildRequired,
    /// The DM-only extractor was used on an interaction invoked within a guild
    #[error("DM-only interaction run inside guild")]
    DmRequired,
}

trait Describe {
    type Desc: fmt::Debug;

    fn describe(&self) -> Self::Desc;
}

type Result<T> = std::result::Result<T, Error>;

/// Core logic common to all interaction visitors
#[derive(Debug)]
pub struct BasicVisitor<'a, I> {
    // TODO: make this private once dedicated interaction visitors are done
    pub(crate) int: &'a I,
}

impl<'a, I: private::Interaction> BasicVisitor<'a, I> {
    /// Visit the source guild information for this interaction
    ///
    /// # Errors
    /// This method returns an error if the input data is non-conformant.
    #[inline]
    pub fn guild(&self) -> Result<GuildVisitor> {
        if self.int.guild_id().is_some() != self.int.member().is_some() {
            return Err(Error::Malformed(
                "Guild ID and member info presence desynced",
            ));
        }

        Ok(GuildVisitor(
            self.int.guild_id().zip(self.int.member().as_ref()),
        ))
    }

    /// Visit the invoking user information for this interaction
    #[inline]
    #[must_use]
    pub fn user(&self) -> &'a User { self.int.user() }

    /// Get the ID used by Discord to identify this interaction
    #[inline]
    #[must_use]
    pub fn id(&self) -> InteractionId { *self.int.id() }
}

/// Visitor for the source guild of an interaction
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct GuildVisitor<'a>(Option<(GuildId, &'a Member)>);

// TODO: handle the dm_permission field
impl<'a> GuildVisitor<'a> {
    /// Extract the guild ID and member info, returning `None` if no guild data
    /// is present
    #[inline]
    #[must_use]
    pub fn optional(self) -> Option<(GuildId, &'a Member)> { self.0 }

    /// Extract the guild ID and member info
    ///
    /// # Errors
    /// This method returns an error if no guild data is present.
    #[inline]
    pub fn required(self) -> Result<(GuildId, &'a Member)> { self.0.ok_or(Error::GuildRequired) }

    /// Verify the interaction occurred outside a guild
    ///
    /// # Errors
    /// This method returns an error if guild data is present.
    #[inline]
    pub fn require_dm(self) -> Result<()> {
        self.0.is_none().then_some(()).ok_or(Error::DmRequired)
    }
}

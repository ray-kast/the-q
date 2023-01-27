use serenity::model::{
    application::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::{
            ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
            CommandDataResolved,
        },
    },
    channel::{Attachment, Message, PartialChannel},
    guild::{Member, PartialMember, Role},
    id::GuildId,
    user::User,
};

use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // Top-level command type errors
    #[error("Attempted to read options for a non-slash command")]
    NotChatInput,
    #[error("Attempted to read target user for a non-user command")]
    NotUser,
    #[error("Attempted to read target message for a non-message command")]
    NotMessage,

    // Option visitor errors
    #[error("Command option {0:?} not provided or already visited")]
    MissingOption(String),
    #[error("No value for required command option {0:?}")]
    MissingOptionValue(String),
    #[error("Command option type mismatch - expected {1}, found {2:?}")]
    BadOptionType(String, &'static str, CommandOptionType),
    #[error("Type mismatch in value of command option {0:?} - expected {1}, found {2:?}")]
    BadOptionValueType(String, &'static str, OptionValueType),
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

#[derive(Debug)]
pub enum OptionValueType {
    String,
    Integer,
    Boolean,
    User,
    Channel,
    Role,
    Number,
    Attachment,
    Unknown,
}

impl Describe for CommandDataOptionValue {
    type Desc = OptionValueType;

    fn describe(&self) -> Self::Desc {
        match self {
            Self::String(_) => OptionValueType::String,
            Self::Integer(_) => OptionValueType::Integer,
            Self::Boolean(_) => OptionValueType::Boolean,
            Self::User(..) => OptionValueType::User,
            Self::Channel(_) => OptionValueType::Channel,
            Self::Role(_) => OptionValueType::Role,
            Self::Number(_) => OptionValueType::Number,
            Self::Attachment(_) => OptionValueType::Attachment,
            _ => OptionValueType::Unknown,
        }
    }
}

type Result<T> = std::result::Result<T, Error>;
type OptionMap<'a> = HashMap<&'a str, &'a CommandDataOption>;

enum VisitorState<'a> {
    Init,
    SlashCommand(OptionMap<'a>),
}

pub struct Visitor<'a> {
    aci: &'a ApplicationCommandInteraction,
    state: VisitorState<'a>,
}

macro_rules! ensure_opt_type {
    ($name:expr, $opt:expr, $ty:pat, $desc:literal) => {
        match $opt.kind {
            $ty => (),
            t => return Err(Error::BadOptionType($name, $desc, t)),
        }
    };
}

macro_rules! resolve_opt {
    ($name:expr, $opt:expr, $ty:pat => $val:expr, $desc:literal) => {{
        let val = match &$opt.resolved {
            Some($ty) => Ok(Some($val)),
            Some(v) => Err(Error::BadOptionValueType($name.into(), $desc, v.describe())),
            None => Ok(None),
        };
        val.map(|v| OptionVisitor($name, v))
    }};
}

macro_rules! visit_basic {
    () => {};

    (
        #[doc = $desc:literal]
        $vis:vis fn $name:ident() -> $ty:ty { $var:ident($($val:pat),*) => $expr:expr }
        $($tt:tt)*
    ) => {
        $vis fn $name(&mut self, name: &'a str) -> Result<OptionVisitor<$ty>> {
            let opt = self.visit_opt(name)?;
            // TODO: is this necessary?
            ensure_opt_type!(name.into(), opt, CommandOptionType::$var, $desc);
            resolve_opt!(name, opt, CommandDataOptionValue::$var($($val),*) => $expr, $desc)
        }

        visit_basic! { $($tt)* }
    };
}

impl<'a> Visitor<'a> {
    visit_basic! {
        ///a string
        pub fn visit_string() -> &'a String { String(s) => s }

        ///an integer
        pub fn visit_i64() -> i64 { Integer(i) => *i }

        ///a boolean
        pub fn visit_bool() -> bool { Boolean(b) => *b }

        ///a user
        pub fn visit_user() -> (&'a User, &'a Option<PartialMember>) {
            User(u, m) => (u, m)
        }

        ///a channel
        pub fn visit_channel() -> &'a PartialChannel { Channel(c) => c }

        ///a role
        pub fn visit_role() -> &'a Role { Role(r) => r }

        ///a number
        pub fn visit_number() -> f64 { Number(f) => *f }

        ///an attachment
        pub fn visit_attachment() -> &'a Attachment { Attachment(a) => a }
    }

    pub fn new(aci: &'a ApplicationCommandInteraction) -> Self {
        Self {
            aci,
            state: VisitorState::Init,
        }
    }

    fn visit_opts(&mut self) -> Result<&mut OptionMap<'a>> {
        if let VisitorState::SlashCommand(ref mut m) = self.state {
            return Ok(m);
        }

        if !matches!(self.aci.data.kind, CommandType::ChatInput) {
            return Err(Error::NotChatInput);
        }

        let map = self
            .aci
            .data
            .options
            .iter()
            .map(|o| (&*o.name, o))
            .collect();

        self.state = VisitorState::SlashCommand(map);
        let VisitorState::SlashCommand(ref mut m) = self.state else { unreachable!(); };
        Ok(m)
    }

    #[inline]
    fn visit_opt(&mut self, name: &'a str) -> Result<&'a CommandDataOption> {
        self.visit_opts()?
            .remove(&name)
            .ok_or_else(|| Error::MissingOption(name.into()))
    }

    pub fn visit_subcommand<T: FromStr>(&mut self, name: &'a str) -> Result<OptionVisitor<T>>
    where T::Err: std::error::Error + Send + Sync + 'static {
        let opt = self.visit_opt(name)?;
        ensure_opt_type!(
            name.into(),
            opt,
            CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup,
            "a subcommand"
        );

        let val = resolve_opt!(name, opt, CommandDataOptionValue::String(s) => s, "a string")?;

        val.map(|v| v.parse())
            .transpose()
            .map_err(|e: T::Err| Error::OptionParse(name.into(), e.into()))
    }

    #[inline]
    pub fn guild(&self) -> GuildVisitor {
        assert_eq!(self.aci.guild_id.is_some(), self.aci.member.is_some());
        GuildVisitor(self.aci.guild_id.zip(self.aci.member.as_ref()))
    }

    #[inline]
    pub fn user(&self) -> &'a User { &self.aci.user }

    #[inline]
    pub fn target(&self) -> TargetVisitor<'a> {
        TargetVisitor(self.aci.data.kind, &self.aci.data.resolved)
    }

    pub(super) fn finish(self) -> Result<()> {
        let Self { aci, state } = self;

        match state {
            VisitorState::Init => {
                if aci.data.kind == CommandType::ChatInput && !aci.data.options.is_empty() {
                    return Err(Error::Trailing(
                        aci.data.options.iter().map(|o| o.name.clone()).collect(),
                    ));
                }
            },
            VisitorState::SlashCommand(m) => {
                if !m.is_empty() {
                    return Err(Error::Trailing(m.into_keys().map(Into::into).collect()));
                }
            },
        };

        Ok(())
    }
}

pub struct OptionVisitor<'a, T>(&'a str, Option<T>);

impl<'a, T> OptionVisitor<'a, T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> OptionVisitor<'a, U> {
        OptionVisitor(self.0, self.1.map(f))
    }

    pub fn optional(self) -> Option<T> { self.1 }

    pub fn required(self) -> Result<T> {
        self.1
            .ok_or_else(|| Error::MissingOptionValue(self.0.into()))
    }
}

impl<'a, T, E> OptionVisitor<'a, std::result::Result<T, E>> {
    fn transpose(self) -> std::result::Result<OptionVisitor<'a, T>, E> {
        Ok(OptionVisitor(self.0, self.1.transpose()?))
    }
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

pub struct TargetVisitor<'a>(CommandType, &'a CommandDataResolved);

impl<'a> TargetVisitor<'a> {
    fn pull_single_opt<K, V>(
        map: &'a HashMap<K, V>,
        name: &'static str,
    ) -> Result<Option<(&'a K, &'a V)>> {
        let mut it = map.iter();

        let Some(pair) = it.next() else { return Ok(None) };

        it.next()
            .is_none()
            .then_some(Some(pair))
            .ok_or_else(|| Error::Trailing(vec![name.into()]))
    }

    fn pull_single<K, V>(map: &'a HashMap<K, V>, name: &'static str) -> Result<(&'a K, &'a V)> {
        Self::pull_single_opt(map, name)
            .and_then(|o| o.ok_or_else(|| Error::MissingOptionValue(name.into())))
    }

    pub fn user(self) -> Result<(&'a User, Option<&'a PartialMember>)> {
        let (users, members) = (self.0 == CommandType::User)
            .then_some((&self.1.users, &self.1.members))
            .ok_or(Error::NotUser)?;

        let (_uid, user) = Self::pull_single(users, "user")?;
        let memb = Self::pull_single_opt(members, "member")?.map(|(_, m)| m);

        Ok((user, memb))
    }

    pub fn message(self) -> Result<&'a Message> {
        let (_mid, msg) = Self::pull_single(
            (self.0 == CommandType::Message)
                .then_some(&self.1.messages)
                .ok_or(Error::NotMessage)?,
            "message",
        )?;

        Ok(msg)
    }
}

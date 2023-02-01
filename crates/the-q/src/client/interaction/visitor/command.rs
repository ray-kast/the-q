use serenity::model::{
    application::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::{
            CommandData, CommandDataOption, CommandDataOptionValue, CommandDataResolved,
        },
    },
    channel::{Attachment, Message, PartialChannel},
    guild::{PartialMember, Role},
    user::User,
};

use super::{BasicVisitor, Describe, Error, Result};
use crate::prelude::*;

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

type Subcommand<'a> = Vec<&'a str>;
type OptionMap<'a> = HashMap<&'a str, &'a CommandDataOption>;

enum VisitorState<'a> {
    Init,
    SlashCommand(OptionMap<'a>),
}

pub struct CommandVisitor<'a, I> {
    base: BasicVisitor<'a, I>,
    state: VisitorState<'a>,
}

impl<'a, I> CommandVisitor<'a, I> {
    pub fn new(int: &'a I) -> Self {
        Self {
            base: BasicVisitor::new(int),
            state: VisitorState::Init,
        }
    }
}

impl<'a, I> std::ops::Deref for CommandVisitor<'a, I> {
    type Target = BasicVisitor<'a, I>;

    fn deref(&self) -> &Self::Target { &self.base }
}

impl<'a, I> std::ops::DerefMut for CommandVisitor<'a, I> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
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

impl<'a, I: super::private::Interaction<Data = CommandData>> CommandVisitor<'a, I> {
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

    fn visit_opts(&mut self) -> Result<(Option<Subcommand<'a>>, &mut OptionMap<'a>)> {
        if let VisitorState::SlashCommand(ref mut m) = self.state {
            return Ok((None, m));
        }

        if !matches!(self.base.int.data().kind, CommandType::ChatInput) {
            return Err(Error::NotChatInput);
        }

        let mut subcmd = Vec::new();
        let mut opts = self.base.int.data().options.iter().enumerate().peekable();

        while let Some((_, opt)) = opts.next_if(|(i, o)| {
            *i == 0
                && matches!(
                    o.kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                )
        }) {
            subcmd.push(&*opt.name);
            if opts.next().is_some() {
                return Err(Error::Malformed("Found normal option after subcommand"));
            }
            opts = opt.options.iter().enumerate().peekable();
        }

        let map = opts
            .map(|(_, o)| {
                if matches!(
                    o.kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                ) {
                    return Err(Error::Malformed("Found subcommand after normal option(s)"));
                }

                Ok((&*o.name, o))
            })
            .collect::<Result<_>>()?;

        self.state = VisitorState::SlashCommand(map);
        let VisitorState::SlashCommand(ref mut m) = self.state else { unreachable!(); };
        Ok(((!subcmd.is_empty()).then_some(subcmd), m))
    }

    #[inline]
    fn visit_opt(&mut self, name: &'a str) -> Result<&'a CommandDataOption> {
        let (subcmd, opts) = self.visit_opts()?;

        if let Some(subcmd) = subcmd {
            return Err(Error::UnhandledSubcommand(
                subcmd.into_iter().map(Into::into).collect(),
            ));
        }

        opts.remove(&name)
            .ok_or_else(|| Error::MissingOption(name.into()))
    }

    pub fn visit_subcmd(&mut self) -> Result<Subcommand<'a>> {
        let (subcmd, _opts) = self.visit_opts()?;

        subcmd.ok_or(Error::MissingSubcommand)
    }

    #[inline]
    pub fn target(&self) -> TargetVisitor<'a> {
        TargetVisitor(self.base.int.data().kind, &self.base.int.data().resolved)
    }

    pub(in super::super) fn finish(self) -> Result<()> {
        let Self { base, state } = self;

        match state {
            VisitorState::Init => {
                if base.int.data().kind == CommandType::ChatInput
                    && !base.int.data().options.is_empty()
                {
                    return Err(Error::Trailing(
                        base.int
                            .data()
                            .options
                            .iter()
                            .map(|o| o.name.clone())
                            .collect(),
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

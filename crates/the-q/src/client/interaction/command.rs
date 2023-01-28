use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU8,
    ops::RangeInclusive,
};

use ordered_float::{NotNan, OrderedFloat};
use qcore::{build_range::BuildRange, builder};
use serde_json::{Number, Value};
use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommandOption},
    model::{
        application::command::{Command, CommandOption, CommandOptionChoice, CommandOptionType},
        channel::ChannelType,
        id::{ApplicationId, CommandId, CommandVersionId, GuildId},
        prelude::command::CommandType,
    },
};

pub mod prelude {
    pub use super::{ArgBuilderExt as _, CommandInfoExt as _};
}

#[derive(Debug, thiserror::Error)]
#[error("Error converting command: {0}")]
pub struct TryFromError(&'static str);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct RegisteredCommand {
    pub(super) id: CommandId,
    pub(super) app: ApplicationId,
    pub(super) guild: Option<GuildId>,
    pub(super) version: CommandVersionId,
    pub(super) info: CommandInfo,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandInfo {
    name: String,
    can_dm: bool,
    data: Data,
}

impl CommandInfo {
    // TODO: do descriptions support markdown?
    #[inline]
    pub fn slash(name: impl Into<String>, desc: impl Into<String>, args: Args) -> Self {
        let name = name.into();
        let desc = desc.into();
        let Args(trie) = args;
        Self {
            name,
            data: Data::Slash { desc, trie },
            can_dm: true,
        }
    }

    #[inline]
    pub fn build_slash(
        name: impl Into<String>,
        desc: impl Into<String>,
        f: impl FnOnce(ArgBuilder) -> ArgBuilder,
    ) -> Result<Self, TryFromError> {
        Ok(Self::slash(name, desc, f(ArgBuilder::default()).build()?))
    }

    #[inline]
    pub fn user(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name,
            data: Data::User,
            can_dm: true,
        }
    }

    #[inline]
    pub fn message(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name,
            data: Data::Message,
            can_dm: true,
        }
    }

    pub fn name(&self) -> &String { &self.name }

    pub fn build(self, cmd: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
        let Self { name, can_dm, data } = self;
        cmd.name(name).dm_permission(can_dm);

        match data {
            Data::Slash { desc, trie } => {
                cmd.description(desc);
                match trie {
                    Trie::Branch { height, children } => {
                        for pair in children {
                            cmd.create_option(|o| Subcommand::build_child(height, pair, o));
                        }
                    },
                    Trie::Leaf {
                        mut args,
                        arg_order,
                    } => {
                        for (name, arg) in arg_order
                            .into_iter()
                            .map(|a| args.remove_entry(&a).unwrap_or_else(|| unreachable!()))
                        {
                            cmd.create_option(|o| arg.build(name, o));
                        }
                        assert!(args.is_empty());
                    },
                }
                cmd
            },
            Data::User => cmd.kind(CommandType::User),
            Data::Message => cmd.kind(CommandType::Message),
        }
    }
}

#[builder(trait_name = "CommandInfoExt")]
impl CommandInfo {
    pub fn can_dm(&mut self, can_dm: bool) { self.can_dm = can_dm; }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Data {
    Slash { desc: String, trie: Trie },
    User,
    Message,
}

#[derive(Debug, Default)]
pub struct Args(Trie);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Trie {
    Branch {
        height: NonZeroU8,
        children: BTreeMap<String, Subcommand>,
    },
    Leaf {
        args: BTreeMap<String, Arg>,
        arg_order: Vec<String>,
    },
}

impl Default for Trie {
    fn default() -> Self {
        Self::Leaf {
            args: BTreeMap::new(),
            arg_order: vec![],
        }
    }
}

impl Trie {
    fn try_build(opts: impl IntoIterator<Item = CommandOption>) -> Result<Self, TryFromError> {
        opts.into_iter().try_fold(Self::default(), |t, o| {
            let CommandOption {
                kind,
                name,
                description: desc,
                required,
                choices,
                options,
                channel_types,
                min_value,
                max_value,
                min_length,
                max_length,
                autocomplete,
                ..
            } = o;
            // TODO: there's supposed to be an nsfw field

            match (
                t,
                matches!(
                    kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                ),
            ) {
                (Trie::Leaf { args, arg_order }, true) if args.is_empty() => {
                    assert!(arg_order.is_empty());
                    let mut children = BTreeMap::new();
                    let node = Trie::try_build(options)?;
                    children.insert(name, Subcommand { desc, node });
                    let height = children
                        .iter()
                        .map(|c| c.1.node.height())
                        .max()
                        .unwrap_or(0)
                        .checked_add(1)
                        .and_then(NonZeroU8::new)
                        .unwrap_or_else(|| unreachable!());
                    Ok(Trie::Branch { height, children })
                },
                (
                    Trie::Branch {
                        height,
                        mut children,
                    },
                    true,
                ) => {
                    let node = Trie::try_build(options)?;
                    let height = height.max(
                        node.height()
                            .checked_add(1)
                            .and_then(NonZeroU8::new)
                            .unwrap_or_else(|| unreachable!()),
                    );
                    assert!(children.insert(name, Subcommand { desc, node }).is_none());
                    Ok(Trie::Branch { height, children })
                },
                (
                    Trie::Leaf {
                        mut args,
                        mut arg_order,
                    },
                    false,
                ) => {
                    let ty = ArgType::try_build(
                        kind,
                        choices,
                        channel_types,
                        min_value..=max_value,
                        min_length..=max_length,
                        autocomplete,
                    )?;
                    arg_order.push(name.clone());
                    assert!(args.insert(name, Arg { desc, required, ty }).is_none());
                    Ok(Trie::Leaf { args, arg_order })
                },
                (..) => Err(TryFromError(
                    "Command had mixed subcommand and non-subcommand options",
                )),
            }
        })
    }

    #[inline]
    fn height(&self) -> u8 {
        match *self {
            Self::Branch { height, .. } => height.into(),
            Self::Leaf { .. } => 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Subcommand {
    desc: String,
    node: Trie,
}

impl Subcommand {
    fn build(
        self,
        opt: &mut CreateApplicationCommandOption,
    ) -> &mut CreateApplicationCommandOption {
        let Self { desc, node } = self;
        opt.description(desc);

        match node {
            Trie::Branch { height, children } => {
                for pair in children {
                    opt.create_sub_option(|s| Subcommand::build_child(height, pair, s));
                }
            },
            Trie::Leaf {
                mut args,
                arg_order,
            } => {
                for (name, arg) in arg_order
                    .into_iter()
                    .map(|a| args.remove_entry(&a).unwrap_or_else(|| unreachable!()))
                {
                    opt.create_sub_option(|s| arg.build(name, s));
                }
                assert!(args.is_empty());
            },
        }

        opt
    }

    fn build_child(
        height: NonZeroU8,
        (name, cmd): (String, Subcommand),
        opt: &mut CreateApplicationCommandOption,
    ) -> &mut CreateApplicationCommandOption {
        cmd.build(opt.name(name).kind(match height.into() {
            1 => CommandOptionType::SubCommand,
            2 => CommandOptionType::SubCommandGroup,
            _ => unreachable!(),
        }))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Arg {
    desc: String,
    required: bool,
    ty: ArgType,
}

impl Arg {
    #[inline]
    pub fn new(desc: impl Into<String>, required: bool, ty: ArgType) -> Self {
        let desc = desc.into();
        Self { desc, required, ty }
    }

    #[inline]
    fn build(
        self,
        name: String,
        opt: &mut CreateApplicationCommandOption,
    ) -> &mut CreateApplicationCommandOption {
        let Self { desc, required, ty } = self;
        opt.name(name).description(desc).required(required);

        match ty {
            ArgType::String {
                autocomplete,
                min_len,
                max_len,
            } => Self::build_bounds(
                opt.kind(CommandOptionType::String)
                    .set_autocomplete(autocomplete),
                min_len..=max_len,
                CreateApplicationCommandOption::min_length,
                CreateApplicationCommandOption::max_length,
            ),
            ArgType::StringChoice(c) => Self::build_choices(
                opt.kind(CommandOptionType::String),
                c,
                CreateApplicationCommandOption::add_string_choice,
            ),
            ArgType::Int {
                autocomplete,
                min,
                max,
            } => Self::build_bounds(
                opt.kind(CommandOptionType::Integer)
                    .set_autocomplete(autocomplete),
                min..=max,
                CreateApplicationCommandOption::min_int_value,
                CreateApplicationCommandOption::max_int_value,
            ),
            ArgType::IntChoice(c) => Self::build_choices(
                opt.kind(CommandOptionType::Integer),
                #[allow(clippy::cast_possible_truncation)] // serenity type error
                c.into_iter().map(|Choice { name, val }| Choice {
                    name,
                    val: val as i32,
                }),
                CreateApplicationCommandOption::add_int_choice,
            ),
            ArgType::Bool => opt.kind(CommandOptionType::Boolean),
            ArgType::User => opt.kind(CommandOptionType::User),
            ArgType::Channel(c) => opt
                .kind(CommandOptionType::Channel)
                .channel_types(&c.into_iter().collect::<Vec<_>>()),
            ArgType::Role => opt.kind(CommandOptionType::Role),
            ArgType::Mention => opt.kind(CommandOptionType::Mentionable),
            ArgType::Real {
                autocomplete,
                min,
                max,
            } => Self::build_bounds(
                opt.kind(CommandOptionType::Number)
                    .set_autocomplete(autocomplete),
                min.map(Into::into)..=max.map(Into::into),
                CreateApplicationCommandOption::min_number_value,
                CreateApplicationCommandOption::max_number_value,
            ),
            ArgType::RealChoice(c) => Self::build_choices(
                opt.kind(CommandOptionType::Number),
                c.into_iter().map(|Choice { name, val }| Choice {
                    name,
                    val: val.into(),
                }),
                CreateApplicationCommandOption::add_number_choice,
            ),
            ArgType::Attachment => opt.kind(CommandOptionType::Attachment),
        }
    }

    #[inline]
    fn build_bounds<T>(
        opt: &mut CreateApplicationCommandOption,
        bounds: RangeInclusive<Option<T>>,
        min: impl FnOnce(&mut CreateApplicationCommandOption, T) -> &mut CreateApplicationCommandOption,
        max: impl FnOnce(&mut CreateApplicationCommandOption, T) -> &mut CreateApplicationCommandOption,
    ) -> &mut CreateApplicationCommandOption {
        let (min_val, max_val) = bounds.into_inner();
        if let Some(v) = min_val {
            min(opt, v);
        }
        if let Some(v) = max_val {
            max(opt, v);
        }
        opt
    }

    #[inline]
    fn build_choices<T>(
        opt: &mut CreateApplicationCommandOption,
        choices: impl IntoIterator<Item = Choice<T>>,
        choice: impl Fn(
            &mut CreateApplicationCommandOption,
            String,
            T,
        ) -> &mut CreateApplicationCommandOption,
    ) -> &mut CreateApplicationCommandOption {
        for c in choices {
            choice(opt, c.name, c.val);
        }
        opt
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArgType {
    String {
        autocomplete: bool,
        min_len: Option<u16>,
        max_len: Option<u16>,
    },
    StringChoice(Choices<String>),
    Int {
        autocomplete: bool,
        min: Option<i64>,
        max: Option<i64>,
    },
    IntChoice(Choices<i64>),
    Bool,
    User,
    Channel(BTreeSet<ChannelType>),
    Role,
    Mention,
    Real {
        autocomplete: bool,
        min: Option<NotNan<f64>>,
        max: Option<NotNan<f64>>,
    },
    RealChoice(Choices<OrderedFloat<f64>>),
    Attachment,
}

#[inline]
fn massage_int(n: Option<Number>) -> Result<Option<i64>, TryFromError> {
    let Some(n) = n else { return Ok(None) };
    Ok(Some(n.as_i64().ok_or(TryFromError(
        "Couldn't parse numeric value as integer",
    ))?))
}

#[inline]
fn massage_real(n: Option<Number>) -> Result<Option<NotNan<f64>>, TryFromError> {
    let Some(n) = n else { return Ok(None) };
    Ok(Some(n.as_f64().and_then(|n| NotNan::new(n).ok()).ok_or(
        TryFromError("Couldn't parse numeric value as real number"),
    )?))
}

impl ArgType {
    fn try_build(
        ty: CommandOptionType,
        choices: Vec<CommandOptionChoice>,
        chan_types: Vec<ChannelType>,
        val_range: RangeInclusive<Option<Number>>,
        len_range: RangeInclusive<Option<u16>>,
        autocomplete: bool,
    ) -> Result<Self, TryFromError> {
        Ok(match ty {
            CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup => {
                return Err(TryFromError("Cannot parse subcommand as argument"));
            },
            CommandOptionType::String => {
                if choices.is_empty() {
                    let (min_len, max_len) = len_range.into_inner();
                    Self::String {
                        autocomplete,
                        min_len,
                        max_len,
                    }
                } else {
                    Self::StringChoice(
                        choices
                            .into_iter()
                            .map(Choice::try_build)
                            .collect::<Result<_, _>>()?,
                    )
                }
            },
            CommandOptionType::Integer => {
                if choices.is_empty() {
                    let (min, max) = val_range.into_inner();
                    Self::Int {
                        autocomplete,
                        min: massage_int(min)?,
                        max: massage_int(max)?,
                    }
                } else {
                    Self::IntChoice(
                        choices
                            .into_iter()
                            .map(Choice::try_build)
                            .collect::<Result<_, _>>()?,
                    )
                }
            },
            CommandOptionType::Boolean => Self::Bool,
            CommandOptionType::User => Self::User,
            CommandOptionType::Channel => Self::Channel(chan_types.into_iter().collect()),
            CommandOptionType::Role => Self::Role,
            CommandOptionType::Mentionable => Self::Mention,
            CommandOptionType::Number => {
                if choices.is_empty() {
                    let (min, max) = val_range.into_inner();
                    Self::Real {
                        autocomplete,
                        min: massage_real(min)?,
                        max: massage_real(max)?,
                    }
                } else {
                    Self::RealChoice(
                        choices
                            .into_iter()
                            .map(Choice::try_build)
                            .collect::<Result<_, _>>()?,
                    )
                }
            },
            CommandOptionType::Attachment => Self::Attachment,
            _ => return Err(TryFromError("Unknown command option type")),
        })
    }
}

type Choices<T> = BTreeSet<Choice<T>>;

// TODO: are choices unique on name, value, both, or neither?
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Choice<T> {
    name: String,
    val: T,
}

impl<T> Choice<T> {
    #[inline]
    pub fn new(name: impl Into<String>, val: T) -> Self {
        let name = name.into();
        Self { name, val }
    }

    #[inline]
    fn try_build(choice: CommandOptionChoice) -> Result<Self, TryFromError>
    where T: TryFromValue {
        let CommandOptionChoice { name, value, .. } = choice;
        Ok(Self {
            name,
            val: T::try_from_val(value)?,
        })
    }
}

trait TryFromValue: Sized {
    fn try_from_val(val: Value) -> Result<Self, TryFromError>;
}

impl TryFromValue for String {
    fn try_from_val(val: Value) -> Result<Self, TryFromError> {
        if let Value::String(s) = val {
            Ok(s)
        } else {
            Err(TryFromError("Cannot parse non-string value as string"))
        }
    }
}

impl TryFromValue for i64 {
    fn try_from_val(val: Value) -> Result<Self, TryFromError> {
        if let Some(i) = val.as_i64() {
            Ok(i)
        } else {
            Err(TryFromError("Cannot parse non-int value as int"))
        }
    }
}

impl TryFromValue for OrderedFloat<f64> {
    fn try_from_val(val: Value) -> Result<Self, TryFromError> {
        if let Some(f) = val.as_f64() {
            Ok(OrderedFloat(f))
        } else {
            Err(TryFromError("Cannot parse non-float value as float"))
        }
    }
}

#[derive(Debug, Default)]
pub struct ArgBuilder(ArgBuilderState);

#[derive(Debug, Default)]
enum ArgBuilderState {
    #[default]
    Default,
    Leaf(BTreeMap<String, Arg>, Vec<String>),
    Branch(NonZeroU8, BTreeMap<String, Subcommand>),
    Error(&'static str),
}

impl ArgBuilder {
    #[inline]
    fn arg_parts(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        ty: ArgType,
    ) {
        let desc = desc.into();
        self.arg(name, Arg { desc, required, ty });
    }

    fn insert_subcommand(&mut self, name: impl Into<String>, cmd: Subcommand) {
        let name = name.into();

        let height = match &mut self.0 {
            s @ ArgBuilderState::Default => {
                *s = ArgBuilderState::Branch(
                    NonZeroU8::new(1).unwrap_or_else(|| unreachable!()),
                    [(name, cmd)].into_iter().collect(),
                );
                1
            },
            s @ ArgBuilderState::Leaf(..) => {
                *s = ArgBuilderState::Error("Attempted to add subcommand after adding arguments");
                return;
            },
            ArgBuilderState::Branch(h, c) => {
                *h = (*h).max(
                    cmd.node
                        .height()
                        .checked_add(1)
                        .and_then(NonZeroU8::new)
                        .unwrap_or_else(|| unreachable!()),
                );

                if c.insert(name, cmd).is_some() {
                    self.0 = ArgBuilderState::Error("Duplicate subcommand name added");
                    return;
                }

                (*h).into()
            },
            ArgBuilderState::Error(_) => return,
        };

        if height > 2 {
            self.0 = ArgBuilderState::Error("Maximum subcommand nesting depth exceeded");
        }
    }

    pub fn build(self) -> Result<Args, TryFromError> {
        Ok(Args(match self.0 {
            ArgBuilderState::Default => Trie::default(),
            ArgBuilderState::Leaf(args, arg_order) => Trie::Leaf { args, arg_order },
            ArgBuilderState::Branch(height, children) => Trie::Branch { height, children },
            ArgBuilderState::Error(e) => return Err(TryFromError(e)),
        }))
    }
}

#[builder(trait_name = "ArgBuilderExt")]
impl ArgBuilder {
    pub fn arg(&mut self, name: impl Into<String>, arg: Arg) {
        let name = name.into();
        match &mut self.0 {
            s @ ArgBuilderState::Default => {
                *s = ArgBuilderState::Leaf([(name.clone(), arg)].into_iter().collect(), vec![name]);
            },
            ArgBuilderState::Leaf(m, v) => {
                if m.insert(name.clone(), arg).is_some() {
                    self.0 = ArgBuilderState::Error("Duplicate argument name added");
                    return;
                }
                v.push(name);
            },
            s @ ArgBuilderState::Branch(..) => {
                *s = ArgBuilderState::Error("Attempted to add argument after adding subcommands");
                return;
            },
            ArgBuilderState::Error(_) => (),
        }
    }

    pub fn string(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        len: impl BuildRange<u16>,
    ) {
        let (min_len, max_len) = len.build_range().into_inner();
        self.arg_parts(name, desc, required, ArgType::String {
            autocomplete: false,
            min_len,
            max_len,
        });
    }

    pub fn string_choice<C: IntoIterator>(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        choices: C,
    ) where
        C::Item: Into<Choice<String>>,
    {
        let choices = choices.into_iter().map(Into::into).collect();
        self.arg_parts(name, desc, required, ArgType::StringChoice(choices));
    }

    pub fn int(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        range: impl BuildRange<i64>,
    ) {
        let (min, max) = range.build_range().into_inner();
        self.arg_parts(name, desc, required, ArgType::Int {
            autocomplete: false,
            min,
            max,
        });
    }

    pub fn int_choice<C: IntoIterator>(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        choices: C,
    ) where
        C::Item: Into<Choice<i64>>,
    {
        let choices = choices.into_iter().map(Into::into).collect();
        self.arg_parts(name, desc, required, ArgType::IntChoice(choices));
    }

    pub fn bool(&mut self, name: impl Into<String>, desc: impl Into<String>, required: bool) {
        self.arg_parts(name, desc, required, ArgType::Bool);
    }

    pub fn user(&mut self, name: impl Into<String>, desc: impl Into<String>, required: bool) {
        self.arg_parts(name, desc, required, ArgType::User);
    }

    pub fn channel(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        types: impl IntoIterator<Item = ChannelType>,
    ) {
        let types = types.into_iter().collect();
        self.arg_parts(name, desc, required, ArgType::Channel(types));
    }

    pub fn role(&mut self, name: impl Into<String>, desc: impl Into<String>, required: bool) {
        self.arg_parts(name, desc, required, ArgType::Role);
    }

    pub fn mention(&mut self, name: impl Into<String>, desc: impl Into<String>, required: bool) {
        self.arg_parts(name, desc, required, ArgType::Mention);
    }

    pub fn real(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        range: impl BuildRange<f64>,
    ) {
        #![allow(clippy::manual_let_else)] // syn doesn't support let-else
        let (min, max) = range.build_range().into_inner();
        let (min, max) = if let Ok(t) = min
            .map(NotNan::new)
            .transpose()
            .and_then(|r| max.map(NotNan::new).transpose().map(|s| (r, s)))
        {
            t
        } else {
            self.0 = ArgBuilderState::Error("NaN given for real argument bound");
            return;
        };

        self.arg_parts(name, desc, required, ArgType::Real {
            autocomplete: false,
            min,
            max,
        });
    }

    pub fn real_choice<C: IntoIterator>(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        required: bool,
        choices: C,
    ) where
        C::Item: Into<Choice<f64>>,
    {
        let choices = choices
            .into_iter()
            .map(|f| {
                let Choice { name, val } = f.into();
                Choice {
                    name,
                    val: OrderedFloat(val),
                }
            })
            .collect();

        self.arg_parts(name, desc, required, ArgType::RealChoice(choices));
    }

    pub fn attachment(&mut self, name: impl Into<String>, desc: impl Into<String>, required: bool) {
        self.arg_parts(name, desc, required, ArgType::Attachment);
    }

    pub fn autocomplete<'a, Q: Eq + Ord + 'a>(
        &mut self,
        enable: bool,
        keys: impl IntoIterator<Item = &'a Q>,
    ) where
        String: std::borrow::Borrow<Q>,
    {
        match &mut self.0 {
            s @ ArgBuilderState::Default => {
                *s = ArgBuilderState::Error("No argument present to set autocomplete");
            },
            ArgBuilderState::Leaf(m, _) => {
                for key in keys {
                    if let Some(Arg {
                        ty:
                            ArgType::String { autocomplete, .. }
                            | ArgType::Int { autocomplete, .. }
                            | ArgType::Real { autocomplete, .. },
                        ..
                    }) = m.get_mut(key)
                    {
                        *autocomplete = enable;
                    } else {
                        self.0 = ArgBuilderState::Error("Invalid argument for autocomplete");
                        return;
                    }
                }
            },
            s @ ArgBuilderState::Branch(..) => {
                *s = ArgBuilderState::Error("Cannot set autocomplete on a subcommand");
            },
            ArgBuilderState::Error(_) => (),
        }
    }

    pub fn subcmd(&mut self, name: impl Into<String>, desc: impl Into<String>, args: Args) {
        let name = name.into();
        let desc = desc.into();
        let Args(node) = args;
        self.insert_subcommand(name, Subcommand { desc, node });
    }

    #[inline]
    pub fn build_subcmd(
        &mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        f: impl FnOnce(ArgBuilder) -> ArgBuilder,
    ) {
        match f(ArgBuilder::default()).build() {
            Ok(a) => {
                self.subcmd(name, desc, a);
            },
            Err(TryFromError(e)) => self.0 = ArgBuilderState::Error(e),
        }
    }
}

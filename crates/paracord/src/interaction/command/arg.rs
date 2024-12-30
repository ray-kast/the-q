use std::{collections::BTreeSet, ops::RangeInclusive};

use ordered_float::{NotNan, OrderedFloat};
use serde_json::Number;
use serenity::{
    builder::CreateCommandOption,
    model::{
        application::{CommandOptionChoice, CommandOptionType},
        channel::ChannelType,
    },
};

use super::{try_from_value::TryFromValue, TryFromError};

/// Metadata for a chat input command parameter
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Arg {
    pub(super) desc: String,
    pub(super) required: bool,
    pub(super) ty: ArgType,
}

impl Arg {
    /// Construct a new parameter description
    #[inline]
    pub fn new(desc: impl Into<String>, required: bool, ty: ArgType) -> Self {
        let desc = desc.into();
        Self { desc, required, ty }
    }

    #[inline]
    pub(super) fn build(self, name: String) -> CreateCommandOption {
        let Self { desc, required, ty } = self;
        let opt = CreateCommandOption::new(ty.option_ty(), name, desc).required(required);

        match ty {
            ArgType::String {
                autocomplete,
                min_len,
                max_len,
            } => Self::build_bounds(
                opt.set_autocomplete(autocomplete),
                min_len..=max_len,
                CreateCommandOption::min_length,
                CreateCommandOption::max_length,
            ),
            ArgType::StringChoice(c) => {
                Self::build_choices(opt, c, CreateCommandOption::add_string_choice)
            },
            ArgType::Int {
                autocomplete,
                min,
                max,
            } => Self::build_bounds(
                opt.set_autocomplete(autocomplete),
                min..=max,
                // TODO: check for a version of serenity that fixes this
                |c, v| c.min_int_value(v.try_into().expect("Serenity bug encountered")),
                |c, v| c.max_int_value(v.try_into().expect("Serenity bug encountered")),
            ),
            ArgType::IntChoice(c) => Self::build_choices(
                opt,
                #[expect(clippy::cast_possible_truncation, reason = "Serenity type error")]
                c.into_iter().map(|Choice { name, val }| Choice {
                    name,
                    val: val as i32,
                }),
                CreateCommandOption::add_int_choice,
            ),
            ArgType::Channel(c) => opt.channel_types(c.into_iter().collect::<Vec<_>>()),
            ArgType::Real {
                autocomplete,
                min,
                max,
            } => Self::build_bounds(
                opt.set_autocomplete(autocomplete),
                min.map(Into::into)..=max.map(Into::into),
                CreateCommandOption::min_number_value,
                CreateCommandOption::max_number_value,
            ),
            ArgType::RealChoice(c) => Self::build_choices(
                opt,
                c.into_iter().map(|Choice { name, val }| Choice {
                    name,
                    val: val.into(),
                }),
                CreateCommandOption::add_number_choice,
            ),
            ArgType::Bool
            | ArgType::User
            | ArgType::Role
            | ArgType::Mention
            | ArgType::Attachment => opt,
        }
    }

    #[inline]
    fn build_bounds<T>(
        mut opt: CreateCommandOption,
        bounds: RangeInclusive<Option<T>>,
        min: impl FnOnce(CreateCommandOption, T) -> CreateCommandOption,
        max: impl FnOnce(CreateCommandOption, T) -> CreateCommandOption,
    ) -> CreateCommandOption {
        let (min_val, max_val) = bounds.into_inner();
        if let Some(v) = min_val {
            opt = min(opt, v);
        }
        if let Some(v) = max_val {
            opt = max(opt, v);
        }
        opt
    }

    #[inline]
    fn build_choices<T>(
        mut opt: CreateCommandOption,
        choices: impl IntoIterator<Item = Choice<T>>,
        choice: impl Fn(CreateCommandOption, String, T) -> CreateCommandOption,
    ) -> CreateCommandOption {
        // TODO: is there a batch choice mutator?
        for c in choices {
            opt = choice(opt, c.name, c.val);
        }
        opt
    }
}

/// Metadata describing the type of a chat input command parameter
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArgType {
    /// A freeform string
    String {
        /// True if this string parameter should send autocomplete interactions
        autocomplete: bool,
        /// The minimum valid input length
        min_len: Option<u16>,
        /// The maximum valid input length
        max_len: Option<u16>,
    },
    /// A string chosen from a list of options
    StringChoice(Choices<String>),
    /// A freeform integer
    Int {
        /// True if this integer parameter should send autocomplete interactions
        autocomplete: bool,
        /// The minimum valid input value
        min: Option<i64>,
        /// The maximum valid input value
        max: Option<i64>,
    },
    /// An integer chosen from a list of options
    IntChoice(Choices<i64>),
    /// A Boolean parameter
    Bool,
    /// A handle for a user
    User,
    /// A handle for a channel conforming to the list of channel types given
    ///
    /// **NOTE:** If the list provided is empty, all types are assumed to be
    /// valid.
    Channel(BTreeSet<ChannelType>),
    /// A handle for a role within a guild
    Role,
    /// A handle for either a user or a guild role
    Mention,
    /// A freeform real (decimal) number
    Real {
        /// True if this real parameter should send autocomplete interactions
        autocomplete: bool,
        /// The minimum valid input value
        min: Option<NotNan<f64>>,
        /// The maximum valid input value
        max: Option<NotNan<f64>>,
    },
    /// A real (decimal) number chosen from a list of options
    RealChoice(Choices<OrderedFloat<f64>>),
    /// An uploaded attachment
    Attachment,
}

impl ArgType {
    fn option_ty(&self) -> CommandOptionType {
        match self {
            Self::String { .. } | Self::StringChoice(_) => CommandOptionType::String,
            Self::Int { .. } | Self::IntChoice(_) => CommandOptionType::Integer,
            Self::Bool => CommandOptionType::Boolean,
            Self::User => CommandOptionType::User,
            Self::Channel(_) => CommandOptionType::Channel,
            Self::Role => CommandOptionType::Role,
            Self::Mention => CommandOptionType::Mentionable,
            Self::Real { .. } | Self::RealChoice(_) => CommandOptionType::Number,
            Self::Attachment => CommandOptionType::Attachment,
        }
    }
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
    pub(super) fn try_build(
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

/// Metadata for an option for one of the `...Choice` [parameter types](ArgType)
// TODO: are choices unique on name, value, both, or neither?
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Choice<T> {
    pub(super) name: String,
    pub(super) val: T,
}

impl<T> Choice<T> {
    /// Construct a new parameter option
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

use std::{collections::BTreeMap, num::NonZeroU8};

use qcore::builder;
use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::application::{CommandOption, CommandOptionType, CommandType},
};

use super::{Arg, ArgBuilder, ArgType, TryFromError};

/// Metadata for an application command
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandInfo {
    pub(super) name: String,
    pub(super) can_dm: bool,
    pub(super) data: Data,
}

impl CommandInfo {
    /// Construct a new description of a chat input command
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

    /// Construct a new description of a chat input command using the given
    /// closure to build the parameter data
    ///
    /// # Errors
    /// This method returns an error if invoking the closure results in an
    /// [`ArgBuilder`] with an invalid state.
    #[inline]
    pub fn build_slash(
        name: impl Into<String>,
        desc: impl Into<String>,
        // TODO: can these be removed from the API surface?
        f: impl FnOnce(ArgBuilder) -> ArgBuilder,
    ) -> Result<Self, TryFromError> {
        Ok(Self::slash(
            name,
            desc,
            f(ArgBuilder::default()).try_into()?,
        ))
    }

    /// Construct a new description of a user context menu command
    #[inline]
    pub fn user(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name,
            data: Data::User,
            can_dm: true,
        }
    }

    /// Construct a new description of a message context menu command
    #[inline]
    pub fn message(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name,
            data: Data::Message,
            can_dm: true,
        }
    }

    /// Get the unique, non-localized name of this command
    #[inline]
    #[must_use]
    pub fn name(&self) -> &String { &self.name }
}

#[builder(trait_name = CommandInfoExt)]
/// Helper methods for mutating [`CommandInfo`]
impl CommandInfo {
    /// Set whether this command should be usable in DM (i.e. non-guild)
    /// channels
    pub fn can_dm(&mut self, can_dm: bool) { self.can_dm = can_dm; }
}

impl From<CommandInfo> for CreateCommand {
    fn from(value: CommandInfo) -> Self {
        let CommandInfo { name, can_dm, data } = value;
        let cmd = Self::new(name).dm_permission(can_dm);

        match data {
            Data::Slash { desc, trie } => {
                #[repr(transparent)]
                struct Visitor(CreateCommand);

                impl TrieVisitor<CreateCommand> for Visitor {
                    #[inline]
                    fn branch<C: Iterator<Item = CreateCommandOption>>(
                        self,
                        _: NonZeroU8,
                        children: C,
                    ) -> CreateCommand {
                        self.0.set_options(children.collect())
                    }

                    #[inline]
                    fn leaf<C: Iterator<Item = CreateCommandOption>>(
                        self,
                        args: C,
                    ) -> CreateCommand {
                        self.0.set_options(args.collect())
                    }
                }

                trie.visit(Visitor(cmd.description(desc)))
            },
            Data::User => cmd.kind(CommandType::User),
            Data::Message => cmd.kind(CommandType::Message),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Data {
    Slash { desc: String, trie: Trie },
    User,
    Message,
}

/// Metadata for chat input command parameters and/or subcommands
#[derive(Debug, Default)]
pub struct Args(pub(super) Trie);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Trie {
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

trait TrieVisitor<T> {
    fn branch<C: Iterator<Item = CreateCommandOption>>(self, height: NonZeroU8, children: C) -> T;

    fn leaf<C: Iterator<Item = CreateCommandOption>>(self, args: C) -> T;
}

impl Trie {
    fn visit<T, V: TrieVisitor<T>>(self, visitor: V) -> T {
        match self {
            Self::Branch { height, children } => visitor.branch(
                height,
                children
                    .into_iter()
                    .map(|p| Subcommand::build_child(height, p)),
            ),
            Self::Leaf {
                mut args,
                arg_order,
            } => {
                let ret = visitor.leaf(arg_order.into_iter().map(|a| {
                    let (name, arg) = args.remove_entry(&a).unwrap_or_else(|| unreachable!());
                    arg.build(name)
                }));

                if !args.is_empty() {
                    unreachable!("Trailing parameters in trie leaf")
                }

                ret
            },
        }
    }

    pub(super) fn try_build(
        opts: impl IntoIterator<Item = CommandOption>,
    ) -> Result<Self, TryFromError> {
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
    pub(super) fn height(&self) -> u8 {
        match *self {
            Self::Branch { height, .. } => height.into(),
            Self::Leaf { .. } => 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct Subcommand {
    pub(super) desc: String,
    pub(super) node: Trie,
}

impl Subcommand {
    fn build_child(height: NonZeroU8, (name, cmd): (String, Self)) -> CreateCommandOption {
        #[repr(transparent)]
        struct Visitor(CreateCommandOption);

        impl TrieVisitor<CreateCommandOption> for Visitor {
            #[inline]
            fn branch<C: Iterator<Item = CreateCommandOption>>(
                self,
                _: NonZeroU8,
                children: C,
            ) -> CreateCommandOption {
                children.fold(self.0, CreateCommandOption::add_sub_option)
            }

            #[inline]
            fn leaf<C: Iterator<Item = CreateCommandOption>>(self, args: C) -> CreateCommandOption {
                args.fold(self.0, CreateCommandOption::add_sub_option)
            }
        }

        let Self { desc, node } = cmd;
        node.visit(Visitor(CreateCommandOption::new(
            match height.into() {
                1 => CommandOptionType::SubCommand,
                2 => CommandOptionType::SubCommandGroup,
                _ => unreachable!(),
            },
            name,
            desc,
        )))
    }
}

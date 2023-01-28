use std::num::NonZeroU8;

use qcore::builder;
use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommandOption},
    model::{
        application::command::{CommandOption, CommandOptionType},
        prelude::command::CommandType,
    },
};

use super::{Arg, ArgBuilder, ArgType, TryFromError};
use crate::prelude::*;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandInfo {
    pub(super) name: String,
    pub(super) can_dm: bool,
    pub(super) data: Data,
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
pub(super) enum Data {
    Slash { desc: String, trie: Trie },
    User,
    Message,
}

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

impl Trie {
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

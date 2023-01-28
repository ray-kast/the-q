use std::num::NonZeroU8;

use ordered_float::{NotNan, OrderedFloat};
use qcore::{build_range::BuildRange, builder};
use serenity::model::channel::ChannelType;

use super::{Arg, ArgType, Args, Choice, Subcommand, Trie, TryFromError};
use crate::prelude::*;

// TODO: sort through all imports

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

    pub fn autocomplete<'a, Q: Eq + Ord + ?Sized + 'a>(
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

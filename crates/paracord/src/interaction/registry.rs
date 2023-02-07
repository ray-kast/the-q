use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    fmt::{self, Write},
    sync::Arc,
};

use anyhow::Context as _;
use ordered_float::OrderedFloat;
use serenity::{
    client::{Cache, Context},
    model::{
        application::{
            command::{Command, CommandOptionType, CommandType},
            component::ComponentType,
            interaction::{
                application_command::{
                    ApplicationCommandInteraction, CommandData, CommandDataOption,
                    CommandDataOptionValue,
                },
                autocomplete::AutocompleteInteraction,
                message_component::MessageComponentInteraction,
                modal::ModalSubmitInteraction,
            },
        },
        channel::Channel,
        id::{ChannelId, CommandId, GuildId, InteractionId},
        user::User,
    },
};
use tokio::sync::RwLock;

use super::{
    command,
    command::RegisteredCommand,
    handler,
    response::{
        id, prelude::*, BorrowedResponder, BorrowingResponder, InitResponder, Message, ModalSource,
        ResponseError,
    },
    rpc::{ComponentId, Key, ModalId, Schema},
    visitor,
};

#[inline]
fn write_string(f: impl FnOnce(&mut String) -> fmt::Result) -> String {
    let mut s = String::new();
    f(&mut s).unwrap_or_else(|e| unreachable!("{e}"));
    s
}

fn visit_opts(opts: &[CommandDataOption]) -> impl Iterator<Item = &CommandDataOption> {
    let mut stk = vec![opts.iter()];
    std::iter::from_fn(move || {
        loop {
            let it = stk.last_mut()?;
            let Some(next) = it.next() else {
                let _ = stk.pop().unwrap();
                continue;
            };
            stk.push(next.options.iter());
            break Some(next);
        }
    })
}

fn command_name(w: &mut impl Write, cache: &Cache, data: &CommandData) -> fmt::Result {
    match data.kind {
        CommandType::ChatInput => {
            write!(w, "/{}", data.name)?;

            for opt in visit_opts(&data.options) {
                match opt.kind {
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup => {
                        let cmd = match opt.resolved {
                            Some(CommandDataOptionValue::String(ref s)) => s,
                            Some(_) | None => &opt.name,
                        };

                        write!(w, " {cmd}")
                    },
                    _ => {
                        if opt.focused {
                            write!(w, " !:{}(", opt.name)
                        } else {
                            write!(w, " {}(", opt.name)
                        }?;
                        if let Some(ref val) = opt.resolved {
                            match val {
                                CommandDataOptionValue::String(v) => write!(w, "{v:?}"),
                                CommandDataOptionValue::Integer(i) => write!(w, "{i}"),
                                CommandDataOptionValue::Boolean(b) => write!(w, "{b:?}"),
                                CommandDataOptionValue::User(u, _) => {
                                    write!(w, "u:").and_then(|()| write_user(w, u))
                                },
                                CommandDataOptionValue::Channel(c) => {
                                    write!(w, "#{}", c.name.as_deref().unwrap_or("<???>"))
                                },
                                CommandDataOptionValue::Role(r) => {
                                    write!(w, "r:@{}", r.name)
                                },
                                CommandDataOptionValue::Number(f) => write!(w, "{f:.2}"),
                                CommandDataOptionValue::Attachment(a) => {
                                    write!(w, "<{}>", a.filename)
                                },
                                _ => {
                                    write!(w, "<???>")
                                },
                            }?;
                        }
                        write!(w, ")")
                    },
                }?;
            }
            Ok(())
        },
        CommandType::User => {
            write!(w, "user::{} ", data.name)?;
            if data.resolved.users.len() == 1 {
                let user = data
                    .resolved
                    .users
                    .values()
                    .next()
                    .unwrap_or_else(|| unreachable!());
                write_user(w, user)
            } else {
                write!(w, "<target unknown>")
            }
        },
        CommandType::Message => {
            write!(w, "message::{} ", data.name)?;
            if data.resolved.messages.len() == 1 {
                let msg = data
                    .resolved
                    .messages
                    .values()
                    .next()
                    .unwrap_or_else(|| unreachable!());
                write!(w, "/{} in ", msg.id)?;
                write_channel(w, cache, msg.channel_id)
            } else {
                write!(w, "<target unknown>")
            }
        },
        _ => write!(w, "???"),
    }
}

fn command_id(w: &mut impl Write, data: &CommandData, id: InteractionId) -> fmt::Result {
    write!(w, "{id}:{}", data.id)
}

fn write_issuer(
    w: &mut impl Write,
    cache: &Cache,
    user: &User,
    guild: Option<GuildId>,
    chan: ChannelId,
) -> fmt::Result {
    write_user(w, user)?;
    if let Some(gid) = guild {
        if let Some(guild) = cache.guild(gid) {
            write!(w, " in guild {} ", guild.name)
        } else {
            write!(w, " in unknown guild {gid} ")
        }?;
        write_channel(w, cache, chan)
    } else {
        write!(w, " in DM ")?;
        write_channel(w, cache, chan)
    }
}

#[inline]
fn write_user(w: &mut impl Write, u: &User) -> fmt::Result {
    write!(w, "@{}#{:04}", u.name, u.discriminator)
}

#[inline]
fn write_channel(w: &mut impl Write, cache: &Cache, chan: ChannelId) -> fmt::Result {
    if let Some(chan) = cache.channel(chan) {
        match chan {
            Channel::Guild(c) => write!(w, "#{}", c.name),
            Channel::Private(d) => {
                write!(w, "to ")?;
                write_user(w, &d.recipient)
            },
            Channel::Category(c) => write!(w, "[#{}]", c.name),
            _ => write!(w, "#???"),
        }
    } else {
        write!(w, "<#{chan}>")
    }
}

#[inline]
fn aci_name(cache: &Cache, aci: &ApplicationCommandInteraction) -> String {
    write_string(|s| command_name(s, cache, &aci.data))
}

#[inline]
fn aci_id(aci: &ApplicationCommandInteraction) -> String {
    write_string(|s| command_id(s, &aci.data, aci.id))
}

#[inline]
fn aci_issuer(cache: &Cache, aci: &ApplicationCommandInteraction) -> String {
    write_string(|s| write_issuer(s, cache, &aci.user, aci.guild_id, aci.channel_id))
}

#[inline]
fn write_custom_id<T: prost::Message + Default>(w: &mut impl Write, id: &str) -> fmt::Result {
    let parsed = unsafe { id::Id::from_inner(id.into()) };
    let parsed = id::read::<T>(&parsed).ok();

    if let Some(parsed) = parsed {
        write!(w, "{parsed:?}")
    } else {
        write!(w, "{id:?}")
    }
}

#[inline]
fn mc_name<S: Schema>(mc: &MessageComponentInteraction) -> String {
    write_string(|s| {
        let ty = match mc.data.component_type {
            ComponentType::ActionRow => "action_row",
            ComponentType::Button => "button",
            ComponentType::SelectMenu => "combo",
            ComponentType::InputText => "textbox",
            _ => "???",
        };

        write!(s, "{ty}::")?;
        write_custom_id::<S::Component>(s, &mc.data.custom_id)
    })
}

#[inline]
fn mc_id(mc: &MessageComponentInteraction) -> String { format!("{}:{}", mc.id, mc.message.id) }

#[inline]
fn mc_issuer(cache: &Cache, mc: &MessageComponentInteraction) -> String {
    write_string(|s| write_issuer(s, cache, &mc.user, mc.guild_id, mc.channel_id))
}

fn ac_name(cache: &Cache, ac: &AutocompleteInteraction) -> String {
    write_string(|s| command_name(s, cache, &ac.data))
}

fn ac_id(ac: &AutocompleteInteraction) -> String {
    write_string(|s| command_id(s, &ac.data, ac.id))
}

fn ac_issuer(cache: &Cache, ac: &AutocompleteInteraction) -> String {
    write_string(|s| write_issuer(s, cache, &ac.user, ac.guild_id, ac.channel_id))
}

fn ms_name<S: Schema>(ms: &ModalSubmitInteraction) -> String {
    write_string(|s| write_custom_id::<S::Modal>(s, &ms.data.custom_id))
}

fn ms_id(ms: &ModalSubmitInteraction) -> String { ms.id.to_string() }

fn ms_issuer(cache: &Cache, ms: &ModalSubmitInteraction) -> String {
    write_string(|s| write_issuer(s, cache, &ms.user, ms.guild_id, ms.channel_id))
}

type CommandHandler<S> = Arc<dyn handler::CommandHandler<S>>;
type CommandHandlerMap<S> = HashMap<CommandId, CommandHandler<S>>;
type RpcHandler<S, K> = Arc<dyn handler::RpcHandler<S, K>>;
type RpcHandlerMap<S, K> = HashMap<K, RpcHandler<S, K>>;

type ComponentInfo<'a, S> = (
    &'a RpcHandler<S, <S as Schema>::ComponentKey>,
    <S as Schema>::ComponentPayload,
);
type ModalInfo<'a, S> = (
    &'a RpcHandler<S, <S as Schema>::ModalKey>,
    ModalSource,
    <S as Schema>::ModalPayload,
);

/// A self-contained registry of interaction handlers, which can register and
/// dispatch response logic to each handler
#[derive(Debug)]
pub struct Registry<S: Schema> {
    handlers: handler::Handlers<S>,
    commands: RwLock<Option<CommandHandlerMap<S>>>,
    components: RwLock<Option<RpcHandlerMap<S, S::ComponentKey>>>,
    modals: RwLock<Option<RpcHandlerMap<S, S::ModalKey>>>,
}

impl<S: Schema> Registry<S> {
    #[tracing::instrument(level = "info", skip(ctx))]
    async fn patch_commands(
        ctx: &Context,
        init: &handler::Handlers<S>,
        guild: Option<GuildId>,
    ) -> Result<CommandHandlerMap<S>, anyhow::Error> {
        if let Some(guild) = guild {
            todo!("handle guild {guild}");
        }

        let handler::Handlers { commands, .. } = init;
        let mut handlers = HashMap::new();

        let existing = Command::get_global_application_commands(&ctx.http)
            .await
            .context("Error fetching initial command list")?
            .into_iter()
            .map(RegisteredCommand::try_from)
            .collect::<Result<Vec<_>, _>>()
            .context("Error parsing initial command list")?;

        let mut unpaired_existing: HashMap<_, _> = existing.iter().map(|r| (&r.info, r)).collect();

        let mut new: HashMap<_, _> = commands
            .iter()
            .map(|c| {
                let inf = c.register_global();
                (inf.name().clone(), (c, inf))
            })
            .collect();
        assert_eq!(new.len(), commands.len());

        let mut unpaired_new = HashSet::new();

        for (name, (cmd, inf)) in &new {
            if let Some(reg) = unpaired_existing.remove(inf) {
                handlers.insert(reg.id, Arc::clone(cmd));
                continue;
            }

            unpaired_new.insert(name.clone());
        }

        let mut sims: BinaryHeap<_> = unpaired_existing
            .iter()
            .flat_map(|(existing, &reg)| {
                let new = &new;
                unpaired_new.iter().map(move |n| {
                    let (_, new) = new.get(n).unwrap_or_else(|| unreachable!());

                    (
                        OrderedFloat(command::similarity(existing, new)),
                        reg,
                        n.clone(),
                    )
                })
            })
            .collect();

        while let Some((sim, existing, new_name)) = sims.pop() {
            if !unpaired_new.remove(&new_name) || unpaired_existing.remove(&existing.info).is_none()
            {
                continue;
            }

            let (cmd, inf) = new.remove(&new_name).unwrap_or_else(|| unreachable!());
            tracing::info!(
                ?sim,
                id = ?existing.id,
                old = ?existing.info.name(),
                "Updating global command {new_name:?}"
            );
            let res =
                Command::edit_global_application_command(&ctx.http, existing.id, |c| inf.build(c))
                    .await
                    .with_context(|| format!("Error updating command {new_name:?}"))?;
            assert_eq!(existing.id, res.id);
            assert!(handlers.insert(res.id, Arc::clone(cmd)).is_none());
        }

        assert!(unpaired_new.is_empty() || unpaired_existing.is_empty());

        for name in unpaired_new {
            let (cmd, inf) = new.remove(&name).unwrap_or_else(|| unreachable!());
            tracing::info!("Creating global command {name:?}");
            let res = Command::create_global_application_command(&ctx.http, |c| inf.build(c))
                .await
                .with_context(|| format!("Error creating command {name:?}"))?;

            assert!(handlers.insert(res.id, Arc::clone(cmd)).is_none());
        }

        for (inf, reg) in unpaired_existing {
            tracing::info!(
                "Deleting unregistered command {:?} (ID {:?})",
                inf.name(),
                reg.id,
            );
            Command::delete_global_application_command(&ctx.http, reg.id)
                .await
                .with_context(|| format!("Error deleting command {:?}", inf.name()))?;
        }

        assert_eq!(handlers.len(), commands.len());
        Ok(handlers)
    }

    fn collate_rpc<K: Key>(handlers: &[RpcHandler<S, K>]) -> RpcHandlerMap<S, K> {
        let mut map = HashMap::new();

        for handler in handlers {
            for key in handler.register_keys().iter().copied() {
                assert!(map.insert(key, Arc::clone(handler)).is_none());
            }
        }

        map
    }

    fn resolve_command<'a>(
        map: &'a tokio::sync::RwLockReadGuard<'a, Option<CommandHandlerMap<S>>>,
        id: CommandId,
    ) -> Result<&'a CommandHandler<S>, &'static str> {
        let Some(ref map) = **map else {
            tracing::warn!("Rejecting command due to uninitialized registry");
            return Err("Still starting!  Please try again later.");
        };

        let Some(handler) = map.get(&id) else {
            tracing::warn!("Rejecting unknown command");
            return Err("Unknown command - this may be a bug.");
        };

        Ok(handler)
    }

    fn resolve_component<'a>(
        map: &'a tokio::sync::RwLockReadGuard<'a, Option<RpcHandlerMap<S, S::ComponentKey>>>,
        id: &id::Id<'_>,
    ) -> Result<ComponentInfo<'a, S>, &'static str> {
        let Some(ref map) = **map else {
            tracing::warn!("Rejecting component due to uninitialized registry");
            return Err("Still starting!  Please try again later.");
        };

        let payload = match id::read::<S::Component>(id)
            .map_err(Some)
            .and_then(|i| i.try_into_parts().ok_or(None))
        {
            Ok(p) => p,
            Err(Some(err)) => {
                tracing::error!(%err, "Unable to parse component ID");
                return Err("Unrecognized component ID format - this is a bug.");
            },
            Err(None) => {
                tracing::warn!("Rejecting unknown (deprecated?) component ID");
                return Err("Invalid component ID - this feature may have been removed.");
            },
        };

        let Some(handler) = map.get(&(&payload).into()) else {
            tracing::warn!("Rejecting unknown component");
            return Err("Unknown component - this may be a bug.");
        };

        Ok((handler, payload))
    }

    fn resolve_modal<'a>(
        map: &'a tokio::sync::RwLockReadGuard<'a, Option<RpcHandlerMap<S, S::ModalKey>>>,
        id: &id::Id<'_>,
    ) -> Result<ModalInfo<'a, S>, &'static str> {
        let Some(ref map) = **map else {
            tracing::warn!("Rejecting modal due to uninitialized registry");
            return Err("Still starting!  Please try again later.");
        };

        let (source, payload) = match id::read::<S::Modal>(id)
            .map_err(Some)
            .and_then(|i| i.try_into_parts().ok_or(None))
        {
            Ok(p) => p,
            Err(Some(err)) => {
                tracing::error!(%err, "Unable to parse modal ID");
                return Err("Unrecognized modal ID format - this is a bug.");
            },
            Err(None) => {
                tracing::warn!("Rejecting unknown (deprecated?) modal ID");
                return Err("Invalid modal ID - this feature may have been removed.");
            },
        };

        let Some(handler) = map.get(&(&payload).into()) else {
            tracing::warn!("Rejecting unknown modal");
            return Err("Unknown modal - this may be a bug.");
        };

        Ok((handler, source, payload))
    }

    fn pretty_handler_error<'a, I>(
        err: handler::HandlerError<S, I>,
        desc: &'static str,
    ) -> Option<Message<'a, S::Component, id::Error>> {
        match err {
            handler::HandlerError::Parse(err) => match err {
                visitor::Error::GuildRequired => {
                    tracing::debug!(%err, "Responding with guild error");
                    Message::rich(|b| {
                        b.push_bold("ERROR:")
                            .push(" This ")
                            .push(desc)
                            .push(" must be run inside a server.")
                    })
                    .ephemeral(true)
                    .into()
                },
                visitor::Error::DmRequired => {
                    tracing::debug!(%err, "Responding with non-guild error");
                    Message::rich(|b| {
                        b.push_bold("ERROR:")
                            .push(" This ")
                            .push(desc)
                            .push(" cannot be run inside a server.")
                    })
                    .ephemeral(true)
                    .into()
                },
                err => {
                    tracing::error!(%err, "Unexpected error parsing {desc}");
                    Message::rich(|b| {
                        b.push("Unexpected error parsing ")
                            .push(desc)
                            .push(": ")
                            .push_mono_safe(err)
                    })
                    .ephemeral(true)
                    .into()
                },
            },
            handler::HandlerError::User(err, _res) => {
                tracing::debug!(err, "Handler for {desc} responded to user with error");
                None
            },
            handler::HandlerError::Other(err) => {
                tracing::error!(?err, "Unexpected error handling {desc}");
                Message::rich(|b| b.push("Unexpected error: ").push_mono_safe(err))
                    .ephemeral(true)
                    .into()
            },
        }
    }

    /// Construct a new registry from the given set of handlers
    #[must_use]
    pub fn new(handlers: handler::Handlers<S>) -> Self {
        Self {
            handlers,
            commands: None.into(),
            components: None.into(),
            modals: None.into(),
        }
    }

    /// Initialize dispatch logic and register all necessary metadata with
    /// Discord
    ///
    /// # Errors
    /// This method returns an error if an API error response is received during
    /// registration.
    #[inline]
    pub async fn init(&self, ctx: &Context) -> Result<(), anyhow::Error> {
        let mut commands = self.commands.write().await;
        let mut components = self.components.write().await;
        let mut modals = self.modals.write().await;

        *commands = Some(Self::patch_commands(ctx, &self.handlers, None).await?);
        *components = Some(Self::collate_rpc(&self.handlers.components));
        *modals = Some(Self::collate_rpc(&self.handlers.modals));

        // TODO: handle guild commands

        Ok(())
    }

    #[tracing::instrument(level = "error", name = "handle_command", err, skip(self, ctx, aci))]
    async fn try_handle_command(
        &self,
        ctx: &Context,
        aci: ApplicationCommandInteraction,
        name: String,
        id: String,
        issuer: String,
    ) -> Result<(), ResponseError> {
        tracing::info!("Handling application command");

        let map = self.commands.read().await;
        let responder = InitResponder::new(&ctx.http, &aci);
        let handler = match Self::resolve_command(&map, aci.data.id) {
            Ok(h) => h,
            Err(e) => {
                return responder
                    .create_message(Message::plain(e).ephemeral(true))
                    .await
                    .map(|_| ());
            },
        };
        tracing::debug!(?handler, "Command handler selected");

        let mut vis = visitor::CommandVisitor::new(&aci);
        let mut responder = BorrowedResponder::Init(responder);
        let res = handler
            .respond(ctx, &mut vis, BorrowingResponder::new(&mut responder))
            .await;
        let res = res.and_then(|_| vis.finish().map_err(Into::into));

        if let Some(msg) = res
            .err()
            .and_then(|e| Self::pretty_handler_error(e, "command"))
        {
            responder.create_or_followup(msg).await?;
        }

        Ok(())
    }

    #[tracing::instrument(level = "error", name = "handle_component", err, skip(self, ctx, mc))]
    async fn try_handle_component(
        &self,
        ctx: &Context,
        mc: MessageComponentInteraction,
        name: String,
        id: String,
        issuer: String,
    ) -> Result<(), ResponseError> {
        tracing::info!("Handling message component");

        let map = self.components.read().await;
        let responder = InitResponder::new(&ctx.http, &mc);
        let (handler, payload) = match Self::resolve_component(&map, unsafe {
            &id::Id::from_inner(mc.data.custom_id.as_str().into())
        }) {
            Ok(h) => h,
            Err(e) => {
                return responder
                    .create_message(Message::plain(e).ephemeral(true))
                    .await
                    .map(|_| ());
            },
        };
        tracing::debug!(?handler, ?payload, "Component handler selected");

        let mut vis = visitor::BasicVisitor { int: &mc };
        let mut responder = BorrowedResponder::Init(responder);
        let res = handler
            .respond(
                ctx,
                payload,
                &mut vis,
                BorrowingResponder::new(&mut responder),
            )
            .await;

        if let Some(msg) = res
            .err()
            .and_then(|e| Self::pretty_handler_error(e, "component"))
        {
            responder.create_or_followup(msg).await?;
        }

        Ok(())
    }

    #[tracing::instrument(
        level = "error",
        name = "handle_autocomplete",
        err,
        skip(self, ctx, ac)
    )]
    async fn try_handle_autocomplete(
        &self,
        ctx: &Context,
        ac: AutocompleteInteraction,
        name: String,
        id: String,
        issuer: String,
    ) -> Result<(), serenity::Error> {
        tracing::trace!("Handling command autocomplete");

        let map = self.commands.read().await;
        let handler = Self::resolve_command(&map, ac.data.id).ok();

        let mut vis = visitor::CommandVisitor::new(&ac);
        let choices = if let Some(handler) = handler {
            handler
                .complete(ctx, &mut vis)
                .await
                .map_err(|err| tracing::error!(%err, "Error in command completion"))
                .and_then(|c| {
                    serde_json::to_value(c).map_err(
                        |err| tracing::error!(%err, "Error serializing command completions"),
                    )
                })
                .ok()
        } else {
            None
        };
        tracing::trace!(?handler, "Command handler selected");

        ac.create_autocomplete_response(&ctx.http, |ac| {
            if let Some(choices) = choices {
                ac.set_choices(choices)
            } else {
                ac
            }
        })
        .await
    }

    #[tracing::instrument(level = "error", name = "handle_modal", err, skip(self, ctx, ms))]
    async fn try_handle_modal(
        &self,
        ctx: &Context,
        ms: ModalSubmitInteraction,
        name: String,
        id: String,
        issuer: String,
    ) -> Result<(), ResponseError> {
        tracing::info!("Handling modal submit");

        let map = self.modals.read().await;
        let responder = InitResponder::new(&ctx.http, &ms);
        let (handler, src, payload) = match Self::resolve_modal(&map, unsafe {
            &id::Id::from_inner(ms.data.custom_id.as_str().into())
        }) {
            Ok(p) => p,
            Err(e) => {
                return responder
                    .create_message(Message::plain(e).ephemeral(true))
                    .await
                    .map(|_| ());
            },
        };
        tracing::debug!(?handler, ?src, ?payload, "Modal handler selected");
        let src = src; // TODO: use this

        let mut vis = visitor::BasicVisitor { int: &ms };
        let mut responder = BorrowedResponder::Init(responder);
        let res = handler
            .respond(
                ctx,
                payload,
                &mut vis,
                BorrowingResponder::new(&mut responder),
            )
            .await;

        if let Some(msg) = res
            .err()
            .and_then(|e| Self::pretty_handler_error(e, "modal"))
        {
            responder.create_or_followup(msg).await?;
        }

        Ok(())
    }

    /// Dispatch a command interaction to the proper handler and submit a
    /// response
    #[inline]
    pub async fn handle_command(&self, ctx: &Context, aci: ApplicationCommandInteraction) {
        let cache = &ctx.cache;
        let (name, id, iss) = (aci_name(cache, &aci), aci_id(&aci), aci_issuer(cache, &aci));
        self.try_handle_command(ctx, aci, name, id, iss).await.ok();
    }

    /// Dispatch a component interaction to the proper handler and submit a
    /// response
    #[inline]
    pub async fn handle_component(&self, ctx: &Context, mc: MessageComponentInteraction) {
        let cache = &ctx.cache;
        let (name, id, iss) = (mc_name::<S>(&mc), mc_id(&mc), mc_issuer(cache, &mc));
        self.try_handle_component(ctx, mc, name, id, iss).await.ok();
    }

    /// Dispatch an autocomplete interaction to the proper handler and submit a
    /// response
    #[inline]
    pub async fn handle_autocomplete(&self, ctx: &Context, ac: AutocompleteInteraction) {
        let cache = &ctx.cache;
        let (name, id, iss) = (ac_name(cache, &ac), ac_id(&ac), ac_issuer(cache, &ac));
        self.try_handle_autocomplete(ctx, ac, name, id, iss)
            .await
            .ok();
    }

    /// Dispatch a modal-submit interaction to the proper handler and submit a
    /// response
    #[inline]
    pub async fn handle_modal(&self, ctx: &Context, ms: ModalSubmitInteraction) {
        let cache = &ctx.cache;
        let (name, id, iss) = (ms_name::<S>(&ms), ms_id(&ms), ms_issuer(cache, &ms));
        self.try_handle_modal(ctx, ms, name, id, iss).await.ok();
    }
}

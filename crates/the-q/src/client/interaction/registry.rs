// !TODO: stop saying "Failed to"

use std::fmt::Write;

use serenity::{
    client::Context,
    model::{
        application::{
            command::{Command, CommandOptionType, CommandType},
            component::ComponentType,
            interaction::{
                application_command::{
                    ApplicationCommandInteraction, CommandData, CommandDataOptionValue,
                },
                autocomplete::AutocompleteInteraction,
                message_component::MessageComponentInteraction,
                modal::ModalSubmitInteraction,
            },
        },
        id::{CommandId, GuildId, InteractionId},
        user::User,
    },
};

use super::{
    handler,
    response::{BorrowedResponder, BorrowingResponder, InitResponder, Message, MessageOpts},
    visitor,
};
use crate::prelude::*;

#[inline]
fn write_string(f: impl FnOnce(&mut String) -> fmt::Result) -> String {
    let mut s = String::new();
    f(&mut s).unwrap_or_else(|e| unreachable!("{e}"));
    s
}

fn command_name(w: &mut impl Write, data: &CommandData) -> fmt::Result {
    match data.kind {
        CommandType::ChatInput => {
            write!(w, "/{}", data.name)?;

            for opt in &data.options {
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
        CommandType::User => write!(w, "user::{}", data.name),
        CommandType::Message => write!(w, "message::{}", data.name),
        _ => write!(w, "???"),
    }
}

fn command_id(w: &mut impl Write, data: &CommandData, id: InteractionId) -> fmt::Result {
    write!(w, "{id}:{}", data.id)
}

fn command_issuer(w: &mut impl Write, user: &User, guild: &Option<GuildId>) -> fmt::Result {
    write_user(w, user)?;
    write!(w, " {}", guild_src(guild))
}

#[inline]
fn guild_src(id: &Option<GuildId>) -> &'static str {
    if id.is_some() { "in guild" } else { "in DM" }
}

#[inline]
fn write_user(w: &mut impl Write, u: &User) -> fmt::Result {
    write!(w, "@{}#{:04}", u.name, u.discriminator)
}

#[inline]
fn aci_name(aci: &ApplicationCommandInteraction) -> String {
    write_string(|s| command_name(s, &aci.data))
}

#[inline]
fn aci_id(aci: &ApplicationCommandInteraction) -> String {
    write_string(|s| command_id(s, &aci.data, aci.id))
}

#[inline]
fn aci_issuer(aci: &ApplicationCommandInteraction) -> String {
    write_string(|s| command_issuer(s, &aci.user, &aci.guild_id))
}

#[inline]
fn mc_name(mc: &MessageComponentInteraction) -> String {
    let ty = match mc.data.component_type {
        ComponentType::ActionRow => "action_row",
        ComponentType::Button => "button",
        ComponentType::SelectMenu => "combo",
        ComponentType::InputText => "textbox",
        _ => "???",
    };

    format!("{ty}::{}", mc.data.custom_id)
}

#[inline]
fn mc_id(mc: &MessageComponentInteraction) -> String { format!("{}:{}", mc.id, mc.message.id) }

#[inline]
fn mc_issuer(mc: &MessageComponentInteraction) -> String {
    write_string(|s| {
        write_user(s, &mc.user)?;
        write!(s, " {} channel {}", guild_src(&mc.guild_id), mc.channel_id)
    })
}

fn ac_name(ac: &AutocompleteInteraction) -> String { write_string(|s| command_name(s, &ac.data)) }

fn ac_id(ac: &AutocompleteInteraction) -> String {
    write_string(|s| command_id(s, &ac.data, ac.id))
}

fn ac_issuer(ac: &AutocompleteInteraction) -> String {
    write_string(|s| command_issuer(s, &ac.user, &ac.guild_id))
}

fn ms_name(ms: &ModalSubmitInteraction) -> String { ms.data.custom_id.clone() }

fn ms_id(ms: &ModalSubmitInteraction) -> String { ms.id.to_string() }

fn ms_issuer(ms: &ModalSubmitInteraction) -> String {
    write_string(|s| {
        write_user(s, &ms.user)?;
        write!(s, " {}", guild_src(&ms.guild_id))
    })
}

type Handler = Arc<dyn handler::CommandHandler>;
type HandlerMap = HashMap<CommandId, Handler>;

#[derive(Debug)]
struct RegistryInit {
    opts: handler::Opts,
    list: Vec<Handler>,
}

#[derive(Debug)]
pub struct Registry {
    init: RegistryInit,
    map: tokio::sync::RwLock<Option<HandlerMap>>,
}

impl Registry {
    #[instrument(level = "debug", skip(ctx))]
    async fn patch_commands(ctx: &Context, init: &RegistryInit) -> Result<HandlerMap> {
        let RegistryInit { opts, list } = init;
        let mut handlers = HashMap::new();

        let cmds = Command::get_global_application_commands(&ctx.http)
            .await
            .context("Failed to get initial command list")?;

        let mut existing: HashMap<_, _> = cmds
            .iter()
            .map(|c| ((Borrowed(c.name.as_str()), c.kind, c.guild_id), Borrowed(c)))
            .collect();

        for cmd in list {
            let mut builder = serenity::builder::CreateApplicationCommand::default();
            let scope = cmd.register(opts, &mut builder);
            let builder = builder;

            let name = builder
                .0
                .get("name")
                .and_then(serde_json::Value::as_str)
                .expect("Missing command name!")
                .to_owned();
            let kind = builder.0.get("type").expect("Missing command type!");
            let kind = serde_json::from_value(kind.clone()).expect("Invalid command type!");

            let id = existing
                .get(&(Borrowed(name.as_str()), kind, scope))
                .map(|c| c.id);
            let map = serde_json::Value::from(serenity::json::hashmap_to_json_map(builder.0));

            // TODO: skip HTTP request for identical commands

            // Shoutout to serenity for not having non-builder command methods
            let res = match (id, scope) {
                (None, None) => {
                    debug!("Creating global command {name:?}");
                    ctx.http.create_global_application_command(&map).await
                },
                (None, Some(guild)) => {
                    debug!("Creating guild command {name:?} for {guild:?}");
                    ctx.http
                        .create_guild_application_command(guild.into(), &map)
                        .await
                },
                (Some(id), None) => {
                    debug!("Updating global command {name:?} (ID {id:?})");
                    ctx.http
                        .edit_global_application_command(id.into(), &map)
                        .await
                },
                (Some(id), Some(guild)) => {
                    debug!("Updating guild command {name:?} (ID {id:?}) for {guild:?}");
                    ctx.http
                        .edit_guild_application_command(guild.into(), id.into(), &map)
                        .await
                },
            }
            .with_context(|| format!("Failed to upsert {cmd:?}"))?;

            let was_updated = id.is_some();
            let id = res.id;
            let prev = existing.insert(
                (Owned(res.name.clone()), res.kind, res.guild_id),
                Owned(res),
            );

            assert_eq!(prev.is_some(), was_updated);
            assert!(prev.map_or(true, |v| matches!(v, Borrowed(_))));

            assert!(handlers.insert(id, cmd.clone()).is_none());
        }

        for cmd in cmds {
            if handlers.contains_key(&cmd.id) {
                continue;
            }

            debug!(
                "Deleting unregistered command {:?} (ID {:?})",
                cmd.name, cmd.id
            );

            Command::delete_global_application_command(&ctx.http, cmd.id)
                .await
                .with_context(|| format!("Failed to delete command {cmd:?}"))?;
        }

        Ok(handlers)
    }

    pub fn new(opts: handler::Opts, list: Vec<Handler>) -> Self {
        Self {
            init: RegistryInit { opts, list },
            map: None.into(),
        }
    }

    #[inline]
    pub async fn init(&self, ctx: &Context) -> Result {
        let mut state = self.map.write().await;

        *state = Some(Self::patch_commands(ctx, &self.init).await?);

        Ok(())
    }

    #[instrument(
        level = "error",
        name = "handle_command",
        err,
        skip(self, ctx, aci),
        fields(
            // TODO: don't do stringification if logs don't happen
            name = aci_name(&aci),
            id = aci_id(&aci),
            issuer = aci_issuer(&aci),
        ),
    )]
    async fn try_handle_command(
        &self,
        ctx: &Context,
        aci: ApplicationCommandInteraction,
    ) -> Result<(), serenity::Error> {
        trace!("Handling application command");

        let map = self.map.read().await;
        let responder = InitResponder::new(&ctx.http, &aci);

        let Some(ref map) = *map else {
            warn!("Rejecting command due to uninitialized registry");

            return responder
                .create_message(
                    Message::plain("Still starting!  Please try again later.")
                        .ephemeral(true),
                )
                .await
                .map(|_| ());
        };

        let Some(handler) = map.get(&aci.data.id) else {
            warn!("Rejecting unknown command");

            return responder
                .create_message(
                    Message::plain("Unknown command - this may be a bug.")
                        .ephemeral(true),
                )
                .await
                .map(|_| ());
        };

        debug!(?handler, "Handling command");

        let mut vis = visitor::Visitor::new(&aci);
        let mut responder = BorrowedResponder::Init(responder);
        let res = handler
            .respond(ctx, &mut vis, BorrowingResponder::new(&mut responder))
            .await;
        let res = vis.finish().map_err(Into::into).and(res);

        let msg = match res {
            Ok(_res) => None,
            Err(handler::CommandError::Parse(err)) => match err {
                visitor::Error::GuildRequired => {
                    debug!(%err, "Responding with guild error");
                    Message::rich(|b| {
                        b.push_bold("ERROR:")
                            .push(" This command must be run inside a server.")
                    })
                    .ephemeral(true)
                    .into()
                },
                visitor::Error::DmRequired => {
                    debug!(%err, "Responding with non-guild error");
                    Message::rich(|b| {
                        b.push_bold("ERROR:")
                            .push(" This command cannot be run inside a server.")
                    })
                    .ephemeral(true)
                    .into()
                },
                err => {
                    error!(%err, "Unexpected error parsing command");
                    Message::rich(|b| {
                        b.push("Unexpected error parsing command: ")
                            .push_mono_safe(err)
                    })
                    .ephemeral(true)
                    .into()
                },
            },
            Err(handler::CommandError::User(err, _res)) => {
                debug!(err, "Command responded to user with error");
                None
            },
            Err(handler::CommandError::Other(err)) => {
                error!(?err, "Unexpected error handling command");
                Message::rich(|b| b.push("Unexpected error: ").push_mono_safe(err))
                    .ephemeral(true)
                    .into()
            },
        };

        if let Some(msg) = msg {
            responder.upsert_message(msg).await?;
        }

        Ok(())
    }

    #[instrument(
        level = "error",
        name = "handle_component",
        err,
        skip(self, ctx, mc),
        fields(
            // TODO: don't do stringification if logs don't happen
            name = mc_name(&mc),
            id = mc_id(&mc),
            issuer = mc_issuer(&mc),
        ),
    )]
    async fn try_handle_component(
        &self,
        ctx: &Context,
        mc: MessageComponentInteraction,
    ) -> Result<(), serenity::Error> {
        trace!("Handling message component");
        let responder = InitResponder::new(&ctx.http, &mc);

        // TODO
        responder
            .defer_update(MessageOpts::default())
            .await
            .map(|_| ())
    }

    #[instrument(
        level = "error",
        name = "handle_autocomplete",
        err,
        skip(self, ctx, ac),
        fields(
            // TODO: don't do stringification if logs don't happen
            name = ac_name(&ac),
            id = ac_id(&ac),
            issuer = ac_issuer(&ac),
        ),
    )]
    async fn try_handle_autocomplete(
        &self,
        ctx: &Context,
        ac: AutocompleteInteraction,
    ) -> Result<(), serenity::Error> {
        trace!("Handling command autocomplete");

        ac.create_autocomplete_response(&ctx.http, |ac| {
            ac // TODO
        })
        .await
    }

    #[instrument(
        level = "error",
        name = "handle_modal",
        err,
        skip(self, ctx, ms),
        fields(
            // TODO: don't do stringification if logs don't happen
            name = ms_name(&ms),
            id = ms_id(&ms),
            issuer = ms_issuer(&ms),
        ),
    )]
    async fn try_handle_modal(
        &self,
        ctx: &Context,
        ms: ModalSubmitInteraction,
    ) -> Result<(), serenity::Error> {
        trace!("Handling modal submit");

        let responder = InitResponder::new(&ctx.http, &ms);

        // TODO
        responder
            .defer_update(MessageOpts::default())
            .await
            .map(|_| ())
    }

    #[inline]
    pub async fn handle_command(&self, ctx: &Context, aci: ApplicationCommandInteraction) {
        self.try_handle_command(ctx, aci).await.ok();
    }

    pub async fn handle_component(&self, ctx: &Context, mc: MessageComponentInteraction) {
        self.try_handle_component(ctx, mc).await.ok();
    }

    pub async fn handle_autocomplete(&self, ctx: &Context, ac: AutocompleteInteraction) {
        self.try_handle_autocomplete(ctx, ac).await.ok();
    }

    pub async fn handle_modal(&self, ctx: &Context, ms: ModalSubmitInteraction) {
        self.try_handle_modal(ctx, ms).await.ok();
    }
}

use std::collections::HashMap;

use serenity::{
    client::Context,
    model::{
        application::{
            command::{Command, CommandOptionType, CommandType},
            interaction::{
                application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
                InteractionResponseType,
            },
        },
        id::CommandId,
    },
    utils::MessageBuilder,
};

use super::handler;
use crate::prelude::*;

#[inline]
fn aci_name(aci: &ApplicationCommandInteraction) -> String {
    match aci.data.kind {
        CommandType::ChatInput => {
            use fmt::Write;

            let mut s = format!("/{}", aci.data.name);

            for opt in &aci.data.options {
                match opt.kind {
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup => {
                        let cmd = match opt.resolved {
                            Some(CommandDataOptionValue::String(ref s)) => s,
                            Some(_) | None => &opt.name,
                        };

                        write!(s, " {cmd}").unwrap();
                    },
                    _ => {
                        write!(s, " {}(", opt.name).unwrap();
                        if let Some(ref val) = opt.resolved {
                            match val {
                                CommandDataOptionValue::String(v) => write!(s, "{v:?}"),
                                CommandDataOptionValue::Integer(i) => write!(s, "{i}"),
                                CommandDataOptionValue::Boolean(b) => write!(s, "{b:?}"),
                                CommandDataOptionValue::User(u, _) => {
                                    write!(s, "u:@{}#{}", u.name, u.discriminator)
                                },
                                CommandDataOptionValue::Channel(c) => {
                                    write!(s, "#{}", c.name.as_deref().unwrap_or("<???>"))
                                },
                                CommandDataOptionValue::Role(r) => {
                                    write!(s, "r:@{}", r.name)
                                },
                                CommandDataOptionValue::Number(f) => write!(s, "{f:.2}"),
                                CommandDataOptionValue::Attachment(a) => {
                                    write!(s, "<{}>", a.filename)
                                },
                                _ => {
                                    s.push_str("<???>");
                                    Ok(())
                                },
                            }
                            .unwrap();
                        }
                        s.push(')');
                    },
                }
            }

            s
        },
        CommandType::User => format!("user::{}", aci.data.name),
        CommandType::Message => format!("message::{}", aci.data.name),
        _ => "???".into(),
    }
}

#[inline]
fn aci_id(aci: &ApplicationCommandInteraction) -> String { format!("{}:{}", aci.id, aci.data.id) }

type HandlerMap = HashMap<CommandId, Box<dyn handler::Handler>>;

#[derive(Debug)]
struct RegistryInit {
    opts: handler::Opts,
    list: Vec<Box<dyn handler::Handler>>,
}

#[derive(Debug)]
enum RegistryState {
    Uninit(RegistryInit),
    Init(HandlerMap),
    Poison,
}

impl RegistryState {
    fn init(&self) -> Option<&HandlerMap> {
        match self {
            Self::Uninit { .. } | Self::Poison => None,
            Self::Init(m) => Some(m),
        }
    }
}

#[derive(Debug)]
pub struct Registry(tokio::sync::RwLock<RegistryState>);

impl Registry {
    #[instrument(level = "debug", skip(ctx))]
    async fn patch_commands(ctx: &Context, init: RegistryInit) -> Result<HandlerMap> {
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
            let scope = cmd.register(&opts, &mut builder);
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

            assert!(handlers.insert(id, cmd).is_none());
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

    pub fn new(opts: handler::Opts, list: Vec<Box<dyn handler::Handler>>) -> Self {
        Self(RegistryState::Uninit(RegistryInit { opts, list }).into())
    }

    pub async fn init(&self, ctx: &Context) -> Result {
        let mut state = self.0.write().await;
        let RegistryState::Uninit(init) =
            mem::replace(&mut *state, RegistryState::Poison)
        else {
            bail!("Command registry already initialized!");
        };

        *state = RegistryState::Init(Self::patch_commands(ctx, init).await?);

        Ok(())
    }

    pub async fn handle(&self, ctx: &Context, aci: ApplicationCommandInteraction) {
        fn fatal<E: std::error::Error + Send + Sync + 'static>(res: Result<(), E>) {
            match res {
                Ok(()) => (),
                Err(e) => error!(%e, "Fatal error handling interaction"),
            }
        }

        // TODO: don't do stringification if logs don't happen
        let span = error_span!("handle_aci", name = aci_name(&aci), id = aci_id(&aci));
        async move {
            let state = self.0.read().await;
            let Some(cmds) = state.init() else {
                warn!("Rejecting command due to uninitialized registry");

                fatal(
                    aci.create_interaction_response(&ctx.http, |res| {
                        res.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|msg| {
                                msg.content("Still starting!  Please try again later.")
                                    .ephemeral(true)
                            })
                    })
                    .await,
                );

                return;
            };

            let Some(handler) = cmds.get(&aci.data.id) else {
                warn!("Rejecting unknown command");

                fatal(
                    aci.create_interaction_response(&ctx.http, |res| {
                        res.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|msg| {
                                msg.content("Unknown command - this may be a bug.")
                                    .ephemeral(true)
                            })
                    })
                    .await,
                );

                return;
            };

            debug!(?handler, "Handling command");

            // TODO: don't let the handler have the ACI
            match handler.respond(ctx, &aci).await {
                Ok(handler::Response::Message | handler::Response::Modal) => (),
                Err(handler::Error::Responded(err)) => {
                    debug!(err);
                },
                Err(handler::Error::NoResponse(err)) => {
                    error!(?err, "Unexpected error handling command");
                    fatal(
                        aci.create_interaction_response(&ctx.http, |res| {
                            res.kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|msg| {
                                    msg.content(
                                        MessageBuilder::new()
                                            .push("Unexpected error: ")
                                            .push_mono_safe(err)
                                            .build(),
                                    )
                                    .ephemeral(true)
                                })
                        })
                        .await,
                    );
                },
            }
        }
        .instrument(span)
        .await;
    }
}

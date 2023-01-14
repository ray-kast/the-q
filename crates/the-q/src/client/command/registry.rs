use serenity::{
    client::Context,
    model::{
        application::{
            command::{Command, CommandOptionType, CommandType},
            interaction::application_command::{
                ApplicationCommandInteraction, CommandDataOptionValue,
            },
        },
        id::CommandId,
    },
};

use super::{
    handler,
    response::{self, Message},
    visitor,
};
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
                                    write!(s, "u:@{}#{:04}", u.name, u.discriminator)
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

fn aci_issuer(aci: &ApplicationCommandInteraction) -> String {
    let src = if aci.guild_id.is_some() {
        "guild"
    } else {
        "DM"
    };
    format!("@{}#{:04} in {src}", aci.user.name, aci.user.discriminator)
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

    pub async fn init(&self, ctx: &Context) -> Result {
        let mut state = self.map.write().await;

        *state = Some(Self::patch_commands(ctx, &self.init).await?);

        Ok(())
    }

    #[instrument(
        level = "error",
        name = "handle_aci",
        err,
        skip(self, ctx, aci),
        fields(
            // TODO: don't do stringification if logs don't happen
            name = aci_name(&aci),
            id = aci_id(&aci),
            issuer = aci_issuer(&aci),
        ),
    )]
    async fn try_handle_aci(
        &self,
        ctx: &Context,
        aci: ApplicationCommandInteraction,
    ) -> Result<(), serenity::Error> {
        let map = self.map.read().await;
        let responder = response::InitResponder::new(&ctx.http, &aci);

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
        let mut responder = response::BorrowedResponder::Init(responder);
        let res = handler
            .respond(
                ctx,
                &mut vis,
                response::BorrowingResponder::new(&mut responder),
            )
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

    #[inline]
    pub async fn handle_aci(&self, ctx: &Context, aci: ApplicationCommandInteraction) {
        self.try_handle_aci(ctx, aci).await.ok();
    }
}

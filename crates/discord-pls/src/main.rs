//! Test bed for Discord API features

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use rand::prelude::*;
use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::prelude::*,
    prelude::*,
    utils::MessageBuilder,
};
use tracing::Instrument;
use tracing_subscriber::prelude::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
enum InteractionType {
    Command(CommandType),
    MessageComponent,
}

impl InteractionType {
    fn get(int: &Interaction) -> Option<Self> {
        match int {
            Interaction::Command(c) => Some(Self::Command(c.data.kind)),
            Interaction::Component(_) => Some(Self::MessageComponent),
            _ => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
enum FlowType {
    TopLevel(InteractionType),
    ModalSubmit(InteractionType),
}

impl FlowType {
    fn get(int: &Interaction) -> Result<Option<Self>, serde_json::Error> {
        if let Interaction::Modal(m) = int {
            Ok(Some(Self::ModalSubmit(serde_json::from_str(
                &m.data.custom_id,
            )?)))
        } else {
            Ok(InteractionType::get(int).map(Self::TopLevel))
        }
    }

    fn initial_interaction(self) -> InteractionType {
        match self {
            Self::TopLevel(i) | Self::ModalSubmit(i) => i,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
enum ResponseType {
    Message,
    UpdateMessage,
    Modal,
}

impl ResponseType {
    fn create(self) -> RawResponseType {
        match self {
            Self::Message => RawResponseType::ChannelMessageWithSource,
            Self::UpdateMessage => RawResponseType::UpdateMessage,
            Self::Modal => RawResponseType::Modal,
        }
    }

    fn defer(self) -> Option<RawResponseType> {
        match self {
            Self::Message => Some(RawResponseType::DeferredChannelMessageWithSource),
            Self::UpdateMessage => Some(RawResponseType::DeferredUpdateMessage),
            Self::Modal => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
enum RawResponseType {
    ChannelMessageWithSource,
    DeferredChannelMessageWithSource,
    DeferredUpdateMessage,
    UpdateMessage,
    Modal,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
enum CrudOp<T> {
    Create(T),
    Defer(T),
    Edit,
    Delete,
}

impl<T> CrudOp<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> CrudOp<U> {
        match self {
            Self::Create(t) => CrudOp::Create(f(t)),
            Self::Defer(t) => CrudOp::Defer(f(t)),
            Self::Edit => CrudOp::Edit,
            Self::Delete => CrudOp::Delete,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CommandOpt {
    String(String, bool),
    Subcommand(String, Vec<CommandOpt>),
    Group(String, Vec<CommandOpt>),
}

impl CommandOpt {
    fn str(s: impl fmt::Display, r: bool) -> Self { Self::String(s.to_string(), r) }

    fn subcmd(s: impl fmt::Display, o: impl IntoIterator<Item = Self>) -> Self {
        Self::Subcommand(s.to_string(), o.into_iter().collect())
    }

    fn group(s: impl fmt::Display, o: impl IntoIterator<Item = Self>) -> Self {
        Self::Group(s.to_string(), o.into_iter().collect())
    }

    fn build_cmd(opts: &[Self]) -> CreateCommand {
        CreateCommand::new("foo")
            .kind(CommandType::ChatInput)
            .description("a command")
            .set_options(opts.iter().map(CommandOpt::build).collect())
    }

    fn build(&self) -> CreateCommandOption {
        match *self {
            CommandOpt::String(ref s, r) => {
                CreateCommandOption::new(CommandOptionType::String, s, "a string").required(r)
            },

            CommandOpt::Subcommand(ref s, ref o) => o.iter().fold(
                CreateCommandOption::new(CommandOptionType::SubCommand, s, "a subcommand"),
                |o, p| o.add_sub_option(p.build()),
            ),

            CommandOpt::Group(ref s, ref o) => o.iter().fold(
                CreateCommandOption::new(CommandOptionType::SubCommandGroup, s, "a group"),
                |o, p| o.add_sub_option(p.build()),
            ),
        }
    }
}

type CrudResults = BTreeMap<(FlowType, Box<[CrudOp<ResponseType>]>), bool>;

#[expect(
    clippy::large_enum_variant,
    reason = "PingPong is zero-size and the rest are large"
)]
enum TestMode {
    Cartesian {
        untried: BTreeSet<(InteractionType, RawResponseType)>,
        results: BTreeMap<(InteractionType, RawResponseType), bool>,
        modals: bool,
    },
    CrudBrute {
        to_try: BTreeSet<Arc<[CrudOp<usize>]>>,
        untried: BTreeMap<FlowType, Vec<Arc<[CrudOp<usize>]>>>,
        results: CrudResults,
    },
    PingPong,
    CommandReg {
        untried: BTreeSet<Vec<CommandOpt>>,
        results: BTreeMap<Vec<CommandOpt>, bool>,
        cmd: Option<Command>,
    },
}

struct Handler {
    state: tokio::sync::RwLock<TestMode>,
}

fn sample_create_response(
    int_ty: InteractionType,
    res_ty: RawResponseType,
) -> serenity::builder::CreateInteractionResponse {
    match res_ty {
        RawResponseType::ChannelMessageWithSource => {
            serenity::builder::CreateInteractionResponse::Message(
                serenity::builder::CreateInteractionResponseMessage::new()
                    .content("foo")
                    .components(vec![serenity::builder::CreateActionRow::Buttons(vec![
                        serenity::builder::CreateButton::new({
                            let mut id = "\0".to_owned();

                            std::iter::from_fn(|| {
                                Some(rand::thread_rng().gen_range('\0'..char::MAX))
                            })
                            .take(99)
                            .for_each(|c| id.push(c));

                            tracing::trace!(id);

                            id
                        })
                        .style(ButtonStyle::Primary)
                        .label("hi"),
                    ])]),
            )
        },
        RawResponseType::DeferredChannelMessageWithSource => {
            serenity::builder::CreateInteractionResponse::Defer(
                // TODO: can you do anything with this?
                serenity::builder::CreateInteractionResponseMessage::new(),
            )
        },
        RawResponseType::DeferredUpdateMessage => {
            serenity::builder::CreateInteractionResponse::Acknowledge
        },
        RawResponseType::UpdateMessage => {
            serenity::builder::CreateInteractionResponse::UpdateMessage(
                serenity::builder::CreateInteractionResponseMessage::new()
                    .content("bar")
                    .ephemeral(true),
            )
        },
        RawResponseType::Modal => serenity::builder::CreateInteractionResponse::Modal(
            serenity::builder::CreateModal::new(serde_json::to_string(&int_ty).unwrap(), "help")
                .components(vec![serenity::builder::CreateActionRow::InputText(
                    serenity::builder::CreateInputText::new(InputTextStyle::Short, "hi", "hi"),
                )]),
        ),
    }
}

fn sample_edit_response() -> serenity::builder::EditInteractionResponse {
    serenity::builder::EditInteractionResponse::new()
        .content("foo")
        .components(vec![serenity::builder::CreateActionRow::Buttons(vec![
            serenity::builder::CreateButton::new("hi")
                .style(ButtonStyle::Primary)
                .label("hi"),
        ])])
}

async fn create_response<'a>(
    int: &Interaction,
    http: impl CacheHttp,
    res: serenity::builder::CreateInteractionResponse,
) -> serenity::Result<()> {
    match int {
        Interaction::Command(c) => c.create_response(http, res).await,
        Interaction::Component(c) => c.create_response(http, res).await,
        Interaction::Modal(m) => m.create_response(http, res).await,
        _ => unreachable!(),
    }
}

// async fn get_response<F>(
//     int: &interaction::Interaction,
//     http: impl AsRef<serenity::http::Http>,
// ) -> serenity::Result<Message> {
//     match int {
//         interaction::Interaction::Command(aci) => {
//             aci.get_interaction_response(http).await
//         },
//         interaction::Interaction::MessageComponent(mc) => mc.get_interaction_response(http).await,
//         interaction::Interaction::ModalSubmit(ms) => ms.get_interaction_response(http).await,
//         _ => unreachable!(),
//     }
// }

async fn edit_response(
    int: &Interaction,
    http: impl CacheHttp,
    res: serenity::builder::EditInteractionResponse,
) -> serenity::Result<Message> {
    match int {
        Interaction::Command(c) => c.edit_response(http, res).await,
        Interaction::Component(c) => c.edit_response(http, res).await,
        Interaction::Modal(m) => m.edit_response(http, res).await,
        _ => unreachable!(),
    }
}

async fn delete_response(
    int: &Interaction,
    http: impl AsRef<serenity::http::Http>,
) -> serenity::Result<()> {
    match int {
        Interaction::Command(c) => c.delete_response(http).await,
        Interaction::Component(c) => c.delete_response(http).await,
        Interaction::Modal(m) => m.delete_response(http).await,
        _ => unreachable!(),
    }
}

async fn create_followup<'a>(
    int: &Interaction,
    http: impl CacheHttp,
    res: serenity::builder::CreateInteractionResponseFollowup,
) -> serenity::Result<Message> {
    match int {
        Interaction::Command(c) => c.create_followup(http, res).await,
        Interaction::Component(c) => c.create_followup(http, res).await,
        Interaction::Modal(m) => m.create_followup(http, res).await,
        _ => unreachable!(),
    }
}

async fn try_pair(
    int: &Interaction,
    http: impl CacheHttp + AsRef<serenity::http::Http> + Clone,
    int_ty: InteractionType,
    untried: &mut BTreeSet<(InteractionType, RawResponseType)>,
    results: &mut BTreeMap<(InteractionType, RawResponseType), bool>,
) {
    #[tracing::instrument(level = "error", name = "try_pair", skip(f))]
    async fn run(
        int: InteractionType,
        res: RawResponseType,
        f: impl std::future::Future<Output = serenity::Result<()>>,
    ) -> serenity::Result<()> {
        let res = f.await;
        match &res {
            Ok(()) => tracing::info!("Success!"),
            Err(e) => tracing::error!(%e, "Error"),
        }
        res
    }

    let Some(pair) = untried.iter().find(|(t, _)| *t == int_ty).copied() else {
        tracing::warn!(?int_ty, "No untried pairs left");
        return;
    };

    assert!(untried.remove(&pair));
    let (int_ty2, res_ty) = pair;
    assert!(int_ty2 == int_ty);

    let h = http.clone();
    let res = run(int_ty, res_ty, async move {
        create_response(int, h, sample_create_response(int_ty, res_ty)).await
    })
    .await;

    let mut mb = MessageBuilder::new();

    match res {
        Ok(()) => mb.push_bold("Success!"),
        Err(ref e) => mb.push_bold("ERROR: ").push_mono_safe(e.to_string()),
    }
    .push('\n');

    results.insert((int_ty, res_ty), res.is_ok());

    if untried.is_empty() {
        mb.push_bold_line("Test suite completed!");

        let mut s = "Results:".to_owned();

        for ((int, res), success) in results.iter() {
            use std::fmt::Write;

            let success = if *success { "OK" } else { "FAIL" };

            mb.push_mono_safe(format!("{int:?}"))
                .push(" -> ")
                .push_mono_safe(format!("{res:?}"))
                .push(": ")
                .push_bold_line(success);

            let int = format!("{int:?}");
            let res = format!("{res:?}");
            write!(s, "\n{int:<32} -> {res:<32}: {success}").unwrap();
        }

        tracing::info!("{s}");
    } else {
        mb.push_bold_line("Remaining items:");
        for (int, res) in untried.iter() {
            mb.push_mono_safe(format!("{int:?}"))
                .push(" -> ")
                .push_mono_line_safe(format!("{res:?}"));
        }
    }

    if res.is_ok() {
        create_followup(
            int,
            http,
            serenity::builder::CreateInteractionResponseFollowup::new().content(mb.build()),
        )
        .await
        .map(|_| ())
    } else {
        create_response(int, http, {
            serenity::builder::CreateInteractionResponse::Message(
                serenity::builder::CreateInteractionResponseMessage::new().content(mb.build()),
            )
        })
        .await
    }
    .map_err(|err| tracing::warn!(%err, "Sending followup failed"))
    .ok();
}

async fn try_cmdreg(
    int: &Interaction,
    http: impl CacheHttp + AsRef<serenity::http::Http> + Clone,
    untried: &mut BTreeSet<Vec<CommandOpt>>,
    results: &mut BTreeMap<Vec<CommandOpt>, bool>,
    cmd: &mut Option<Command>,
) {
    #[tracing::instrument(level = "error", name = "try_pair", skip(f))]
    async fn run(
        opts: &[CommandOpt],
        f: impl std::future::Future<Output = serenity::Result<Command>>,
    ) -> serenity::Result<Command> {
        let res = f.await;
        match &res {
            Ok(cmd) => tracing::info!(?cmd, "Success!"),
            Err(e) => tracing::error!(%e, "Error"),
        }
        res
    }

    let Some(opts) = untried.pop_first() else {
        tracing::warn!("No untried options left");
        return;
    };

    let h = http.clone();
    let res = run(&opts, async {
        let c = match cmd {
            Some(c) => Command::edit_global_command(h, c.id, CommandOpt::build_cmd(&opts)).await,
            None => {
                Command::create_global_command(h, {
                    let c = CommandOpt::build_cmd(&opts);
                    tracing::debug!(?c);
                    c
                })
                .await
            },
        }?;

        *cmd = Some(c.clone());

        Ok(c)
    })
    .await;

    if let Ok(ref c) = res {
        let mut mb = MessageBuilder::new();
        mb.push_codeblock_safe(format!("{:?}", c.options), None);

        create_response(
            int,
            http.clone(),
            serenity::builder::CreateInteractionResponse::Message(
                serenity::builder::CreateInteractionResponseMessage::new().content(mb.build()),
            ),
        )
        .await
        .map_err(|err| tracing::warn!(%err, "Error sending created command"))
        .ok();
    }

    let mut mb = MessageBuilder::new();

    match res {
        Ok(_) => mb.push_bold("Success!"),
        Err(ref e) => mb.push_bold("ERROR: ").push_mono_safe(e.to_string()),
    }
    .push('\n');

    results.insert(opts, res.is_ok());

    if untried.is_empty() {
        mb.push_bold_line("Test suite completed!");

        let mut s = "Results:".to_owned();

        for (opts, success) in results.iter() {
            use std::fmt::Write;

            let success = if *success { "OK" } else { "FAIL" };

            mb.push_mono_safe(format!("{opts:?}"))
                .push(": ")
                .push_bold_line(success);

            let opts = format!("{opts:?}");
            write!(s, "\n{opts:<74}: {success}").unwrap();
        }

        tracing::info!("{s}");
    } else {
        mb.push_bold_line("Remaining items:");
        for opts in untried.iter() {
            mb.push_mono_line_safe(format!("{opts:?}"));
        }
    }

    if res.is_ok() {
        create_followup(
            int,
            http,
            serenity::builder::CreateInteractionResponseFollowup::new().content(mb.build()),
        )
        .await
        .map(|_| ())
    } else {
        create_response(
            int,
            http,
            serenity::builder::CreateInteractionResponse::Message(
                serenity::builder::CreateInteractionResponseMessage::new().content(mb.build()),
            ),
        )
        .await
    }
    .map_err(|err| tracing::warn!(%err, "Sending followup failed"))
    .ok();
}

fn pick_random<T, R: IntoIterator<Item = T>>(from: &mut Vec<T>, refill: impl FnOnce() -> R) -> T {
    if from.is_empty() {
        from.extend(refill());
    }
    let i = rand::thread_rng().gen_range(0..from.len());
    from.remove(i)
}

fn print_crud_results(results: &CrudResults) -> MessageBuilder {
    use std::fmt::Write;

    let mut mb = MessageBuilder::new();
    mb.push_bold_line("Results:");
    let mut s = "Results:".to_owned();

    for ((flow, ops), success) in results {
        let flow = format!("{flow:?}");
        let ops = format!("{ops:?}");
        let success = if *success { "  OK" } else { "FAIL" };

        mb.push_mono_safe(&flow)
            .push(" :: ")
            .push_mono_safe(&ops)
            .push(": ")
            .push_bold_line(success);

        write!(s, "\n{flow:<32} {ops:<62}  {success}").unwrap();
    }

    tracing::info!("{s}");

    mb
}

#[async_trait::async_trait]
impl EventHandler for Handler {
    #[expect(clippy::too_many_lines, reason = "i mean yea")]
    async fn interaction_create(&self, ctx: Context, int: Interaction) {
        let mut state = self.state.write().await;

        let Some(flow) = FlowType::get(&int).unwrap() else {
            tracing::warn!(kind = ?int.kind(), "Unexpected interaction flow");
            return;
        };

        match *state {
            TestMode::Cartesian {
                ref mut untried,
                ref mut results,
                modals,
            } => match (flow, modals) {
                (FlowType::TopLevel(int_ty), false) | (FlowType::ModalSubmit(int_ty), true) => {
                    try_pair(&int, &ctx.http, int_ty, untried, results).await;
                },
                (FlowType::ModalSubmit(int_ty), false) => {
                    create_response(
                        &int,
                        &ctx.http,
                        sample_create_response(int_ty, RawResponseType::DeferredUpdateMessage),
                    )
                    .await
                    .map_err(|e| tracing::warn!(%e))
                    .ok();
                },
                (FlowType::TopLevel(int_ty), true) => {
                    create_response(
                        &int,
                        &ctx.http,
                        sample_create_response(int_ty, RawResponseType::Modal),
                    )
                    .await
                    .map_err(|e| tracing::error!(%e))
                    .ok();
                },
            },
            TestMode::CrudBrute {
                ref to_try,
                ref mut untried,
                ref mut results,
            } => {
                const TYPES: [ResponseType; 3] = [
                    ResponseType::Message,
                    ResponseType::UpdateMessage,
                    ResponseType::Modal,
                ];

                let untried = untried.entry(flow).or_default();
                let ops = pick_random(untried, || to_try.iter().cloned());

                let mut types = vec![];
                let mut rem_types = vec![];
                let ops = ops
                    .iter()
                    .map(|op| {
                        let defer = matches!(op, CrudOp::Defer(_));
                        op.map(|i| loop {
                            if let Some(t) = types.get(i) {
                                break *t;
                            }

                            types.push(loop {
                                let ty = pick_random(&mut rem_types, || TYPES.iter().copied());

                                if !defer || ty.defer().is_some() {
                                    break ty;
                                }
                            });
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();

                let mut last_type = None;
                let mut res = Ok(());
                for (i, op) in ops.iter().enumerate() {
                    let ty = match op {
                        CrudOp::Create(t) | CrudOp::Defer(t) => {
                            last_type = Some(t);
                            t
                        },
                        CrudOp::Edit | CrudOp::Delete => {
                            if let Some(t) = last_type {
                                t
                            } else {
                                let t = &ResponseType::Message; // TODO

                                last_type = Some(t);
                                t
                            }
                        },
                    };

                    let op_res = match op {
                        CrudOp::Create(_) => {
                            create_response(
                                &int,
                                &ctx.http,
                                sample_create_response(flow.initial_interaction(), ty.create()),
                            )
                            .await
                        },
                        CrudOp::Defer(_) => {
                            create_response(
                                &int,
                                &ctx.http,
                                sample_create_response(
                                    flow.initial_interaction(),
                                    ty.defer().unwrap(),
                                ),
                            )
                            .await
                        },
                        CrudOp::Edit => edit_response(&int, &ctx.http, sample_edit_response())
                            .await
                            .map(|_| ()),
                        CrudOp::Delete => delete_response(&int, &ctx.http).await,
                    };

                    if let Err(e) = op_res {
                        res = Err((e, *op, i));
                        break;
                    }
                }

                let span = tracing::error_span!("try_crud", ?flow, ?ops);
                async move {
                    match res {
                        Ok(()) => tracing::info!("Success!"),
                        Err((ref e, ref op, i)) => tracing::error!(%e, ?op, i, "Error"),
                    }

                    let ok = res.is_ok();
                    let last_ok = results.insert((flow, ops), ok);
                    assert!(last_ok.map_or(true, |o| ok == o));

                    let mut mb = print_crud_results(results);

                    create_followup(
                        &int,
                        &ctx.http,
                        serenity::builder::CreateInteractionResponseFollowup::new()
                            .content(mb.build()),
                    )
                    .await
                    .map_err(|err| tracing::warn!(%err, "Error sending followup"))
                    .ok();
                }
                .instrument(span)
                .await;
            },

            TestMode::PingPong => {
                create_response(
                    &int,
                    &ctx.http,
                    sample_create_response(
                        flow.initial_interaction(),
                        RawResponseType::ChannelMessageWithSource,
                    ),
                )
                .await
                .map_err(|e| tracing::warn!(%e))
                .ok();
            },

            TestMode::CommandReg {
                ref mut untried,
                ref mut results,
                ref mut cmd,
            } => match flow {
                FlowType::TopLevel(InteractionType::Command(CommandType::ChatInput)) => {
                    let Interaction::Command(ref cmd) = int else {
                        unreachable!()
                    };

                    let info = (&cmd.data.options,);
                    tracing::info!(?info);

                    let mut mb = MessageBuilder::new();
                    mb.push_mono_safe(format!("{info:?}"));

                    create_response(
                        &int,
                        &ctx.http,
                        serenity::builder::CreateInteractionResponse::Message(
                            serenity::builder::CreateInteractionResponseMessage::new()
                                .content(mb.build()),
                        ),
                    )
                    .await
                    .map_err(|e| tracing::warn!(%e))
                    .ok();
                },
                FlowType::TopLevel(InteractionType::MessageComponent) => {
                    try_cmdreg(&int, &ctx.http, untried, results, cmd).await;
                },
                f => {
                    create_response(
                        &int,
                        &ctx.http,
                        sample_create_response(
                            f.initial_interaction(),
                            RawResponseType::ChannelMessageWithSource,
                        ),
                    )
                    .await
                    .map_err(|e| tracing::warn!(%e))
                    .ok();
                },
            },
        }
    }
}

#[derive(clap::Parser)]
enum Subcommand {
    Cartesian {
        #[arg(long)]
        modals: bool,
    },
    CrudBrute,
    PingPong,
    CommandReg,
}

#[derive(clap::Parser)]
struct Opts {
    #[arg(long, env = "RUST_LOG")]
    log_filter: Option<String>,

    #[arg(long, env)]
    discord_token: String,

    #[command(subcommand)]
    subcommand: Subcommand,
}

#[tokio::main]
async fn main() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |i| {
        hook(i);
        std::process::abort();
    }));

    [".env.local", ".env.dev", ".env"]
        .into_iter()
        .try_for_each(|p| {
            dotenvy::from_filename(p).map(|_| ()).or_else(|e| match e {
                dotenvy::Error::Io(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                e => Err(e),
            })
        })
        .unwrap();

    let Opts {
        log_filter,
        discord_token,
        subcommand,
    } = clap::Parser::parse();
    let log_filter = log_filter.as_deref().unwrap_or("info");

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_new(log_filter).unwrap())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mode = match subcommand {
        Subcommand::Cartesian { modals } => {
            let int_types = [
                InteractionType::Command(CommandType::ChatInput),
                InteractionType::Command(CommandType::User),
                InteractionType::Command(CommandType::Message),
                InteractionType::MessageComponent,
            ];

            let res_types = [
                RawResponseType::ChannelMessageWithSource,
                RawResponseType::DeferredChannelMessageWithSource,
                RawResponseType::DeferredUpdateMessage,
                RawResponseType::UpdateMessage,
                RawResponseType::Modal,
            ];

            let untried = int_types
                .into_iter()
                .flat_map(|i| res_types.iter().copied().map(move |r| (i, r)))
                .collect();

            TestMode::Cartesian {
                untried,
                results: BTreeMap::default(),
                modals,
            }
        },
        Subcommand::CrudBrute => {
            use CrudOp::{Create, Defer, Delete, Edit};

            let op_lists = [
                vec![Edit],
                vec![Delete],
                vec![Create(0), Edit, Edit],
                vec![Defer(0), Edit, Edit],
                vec![Defer(0), Delete],
                // vec![Defer(0), Delete, Create(0)],
                vec![Defer(0), Defer(0)],
                vec![Delete, Create(0)],
                // vec![Create(0), Delete, Create(1)],
                // vec![Create(0), Create(1)],
            ];

            let to_try = op_lists
                .into_iter()
                .map(|v| v.into_boxed_slice().into())
                .collect();

            TestMode::CrudBrute {
                to_try,
                untried: BTreeMap::default(),
                results: BTreeMap::default(),
            }
        },
        Subcommand::PingPong => TestMode::PingPong,
        Subcommand::CommandReg => {
            use CommandOpt as Opt;

            let untried = [
                // vec![Opt::str("s", true)],
                // vec![Opt::str("s", false)],
                vec![Opt::subcmd("c", [Opt::str("s", true)])],
                vec![Opt::group("hi", [Opt::subcmd("there", [Opt::str(
                    "s", true,
                )])])],
            ]
            .into_iter()
            .collect();

            TestMode::CommandReg {
                untried,
                results: BTreeMap::default(),
                cmd: None,
            }
        },
    };

    let mut client = Client::builder(discord_token, GatewayIntents::non_privileged())
        .event_handler(Handler { state: mode.into() })
        .await
        .unwrap();

    tokio::select! {
        s = tokio::signal::ctrl_c() => s.unwrap(),
        r = client.start() => r.unwrap(),
    }

    client.shard_manager.shutdown_all().await;
}

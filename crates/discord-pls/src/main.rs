#![allow(clippy::all)]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use rand::prelude::*;
use serenity::{model::prelude::*, prelude::*};
use tracing::Instrument;
use tracing_subscriber::prelude::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
enum InteractionType {
    Command(command::CommandType),
    MessageComponent,
}

impl InteractionType {
    fn get(int: &interaction::Interaction) -> Option<Self> {
        match int {
            interaction::Interaction::ApplicationCommand(aci) => Some(Self::Command(aci.data.kind)),
            interaction::Interaction::MessageComponent(_) => Some(Self::MessageComponent),
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
    fn get(int: &interaction::Interaction) -> Result<Option<Self>, serde_json::Error> {
        if let interaction::Interaction::ModalSubmit(ms) = int {
            Ok(Some(Self::ModalSubmit(serde_json::from_str(
                &ms.data.custom_id,
            )?)))
        } else {
            Ok(InteractionType::get(int).map(Self::TopLevel))
        }
    }

    fn initial_interaction(self) -> InteractionType {
        match self {
            Self::TopLevel(i) => i,
            Self::ModalSubmit(i) => i,
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
    fn create(self) -> interaction::InteractionResponseType {
        match self {
            Self::Message => interaction::InteractionResponseType::ChannelMessageWithSource,
            Self::UpdateMessage => interaction::InteractionResponseType::UpdateMessage,
            Self::Modal => interaction::InteractionResponseType::Modal,
        }
    }

    fn defer(self) -> Option<interaction::InteractionResponseType> {
        match self {
            Self::Message => {
                Some(interaction::InteractionResponseType::DeferredChannelMessageWithSource)
            },
            Self::UpdateMessage => {
                Some(interaction::InteractionResponseType::DeferredUpdateMessage)
            },
            Self::Modal => None,
        }
    }
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

enum TestMode {
    Cartesian {
        untried: BTreeSet<(InteractionType, interaction::InteractionResponseType)>,
        results: BTreeMap<(InteractionType, interaction::InteractionResponseType), bool>,
        modals: bool,
    },
    CrudBrute {
        to_try: BTreeSet<Arc<[CrudOp<usize>]>>,
        untried: BTreeMap<FlowType, Vec<Arc<[CrudOp<usize>]>>>,
        results: BTreeMap<(FlowType, Box<[CrudOp<ResponseType>]>), bool>,
    },
}

struct Handler {
    state: tokio::sync::RwLock<TestMode>,
}

fn sample_create_response<'a, 'b>(
    int_ty: InteractionType,
    res_ty: interaction::InteractionResponseType,
    builder: &'a mut serenity::builder::CreateInteractionResponse<'b>,
) -> &'a mut serenity::builder::CreateInteractionResponse<'b> {
    builder.kind(res_ty);
    match res_ty {
        interaction::InteractionResponseType::ChannelMessageWithSource => builder
            .interaction_response_data(|d| {
                d.content("foo").components(|c| {
                    c.create_action_row(|r| {
                        r.create_button(|b| {
                            b.style(component::ButtonStyle::Primary)
                                .label("hi")
                                .custom_id("hi")
                        })
                    })
                })
            }),
        interaction::InteractionResponseType::DeferredChannelMessageWithSource => builder,
        interaction::InteractionResponseType::DeferredUpdateMessage => builder,
        interaction::InteractionResponseType::UpdateMessage => {
            builder.interaction_response_data(|d| d.content("bar").ephemeral(true))
        },
        interaction::InteractionResponseType::Modal => builder.interaction_response_data(|d| {
            d.title("help")
                .custom_id(serde_json::to_string(&int_ty).unwrap())
                .components(|c| {
                    c.create_action_row(|r| {
                        r.create_input_text(|t| {
                            t.style(component::InputTextStyle::Short)
                                .label("hi")
                                .custom_id("hi")
                        })
                    })
                })
        }),
        _ => unreachable!(),
    }
}

fn sample_edit_response<'a>(
    builder: &'a mut serenity::builder::EditInteractionResponse,
) -> &'a mut serenity::builder::EditInteractionResponse {
    builder.content("foo").components(|c| {
        c.create_action_row(|r| {
            r.create_button(|b| {
                b.style(component::ButtonStyle::Primary)
                    .label("hi")
                    .custom_id("hi")
            })
        })
    })
}

async fn create_response<'a, F>(
    int: &interaction::Interaction,
    http: impl AsRef<serenity::http::Http>,
    build: F,
) -> serenity::Result<()>
where
    for<'b> F: FnOnce(
        &'b mut serenity::builder::CreateInteractionResponse<'a>,
    ) -> &'b mut serenity::builder::CreateInteractionResponse<'a>,
{
    match int {
        interaction::Interaction::ApplicationCommand(aci) => {
            aci.create_interaction_response(http, build).await
        },
        interaction::Interaction::MessageComponent(mc) => {
            mc.create_interaction_response(http, build).await
        },
        interaction::Interaction::ModalSubmit(ms) => {
            ms.create_interaction_response(http, build).await
        },
        _ => unreachable!(),
    }
}

async fn get_response<F>(
    int: &interaction::Interaction,
    http: impl AsRef<serenity::http::Http>,
) -> serenity::Result<Message> {
    match int {
        interaction::Interaction::ApplicationCommand(aci) => {
            aci.get_interaction_response(http).await
        },
        interaction::Interaction::MessageComponent(mc) => mc.get_interaction_response(http).await,
        interaction::Interaction::ModalSubmit(ms) => ms.get_interaction_response(http).await,
        _ => unreachable!(),
    }
}

async fn edit_response<F>(
    int: &interaction::Interaction,
    http: impl AsRef<serenity::http::Http>,
    build: F,
) -> serenity::Result<Message>
where
    for<'b> F: FnOnce(
        &'b mut serenity::builder::EditInteractionResponse,
    ) -> &'b mut serenity::builder::EditInteractionResponse,
{
    match int {
        interaction::Interaction::ApplicationCommand(aci) => {
            aci.edit_original_interaction_response(http, build).await
        },
        interaction::Interaction::MessageComponent(mc) => {
            mc.edit_original_interaction_response(http, build).await
        },
        interaction::Interaction::ModalSubmit(ms) => {
            ms.edit_original_interaction_response(http, build).await
        },
        _ => unreachable!(),
    }
}

async fn delete_response(
    int: &interaction::Interaction,
    http: impl AsRef<serenity::http::Http>,
) -> serenity::Result<()> {
    match int {
        interaction::Interaction::ApplicationCommand(aci) => {
            aci.delete_original_interaction_response(http).await
        },
        interaction::Interaction::MessageComponent(mc) => {
            mc.delete_original_interaction_response(http).await
        },
        interaction::Interaction::ModalSubmit(ms) => {
            ms.delete_original_interaction_response(http).await
        },
        _ => unreachable!(),
    }
}

async fn create_followup<'a, F>(
    int: &interaction::Interaction,
    http: impl AsRef<serenity::http::Http>,
    build: F,
) -> serenity::Result<Message>
where
    for<'b> F: FnOnce(
        &'b mut serenity::builder::CreateInteractionResponseFollowup<'a>,
    ) -> &'b mut serenity::builder::CreateInteractionResponseFollowup<'a>,
{
    match int {
        interaction::Interaction::ApplicationCommand(aci) => {
            aci.create_followup_message(http, build).await
        },
        interaction::Interaction::MessageComponent(mc) => {
            mc.create_followup_message(http, build).await
        },
        interaction::Interaction::ModalSubmit(ms) => ms.create_followup_message(http, build).await,
        _ => unreachable!(),
    }
}

async fn try_pair(
    int: &interaction::Interaction,
    http: impl AsRef<serenity::http::Http> + Clone,
    int_ty: InteractionType,
    untried: &mut BTreeSet<(InteractionType, interaction::InteractionResponseType)>,
    results: &mut BTreeMap<(InteractionType, interaction::InteractionResponseType), bool>,
) {
    #[tracing::instrument(level = "error", name = "try_pair", skip(f))]
    async fn run(
        int: InteractionType,
        res: interaction::InteractionResponseType,
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
        create_response(int, h, |res| sample_create_response(int_ty, res_ty, res)).await
    })
    .await;

    let mut mb = serenity::utils::MessageBuilder::new();

    match res {
        Ok(()) => mb.push_bold("Success!"),
        Err(ref e) => mb.push_bold("ERROR: ").push_mono_safe(e),
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
        create_followup(int, http, |res| res.content(mb))
            .await
            .map(|_| ())
    } else {
        create_response(int, http, |res| {
            res.kind(interaction::InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|d| d.content(mb))
        })
        .await
    }
    .map_err(|e| tracing::warn!(%e, "Failed to send followup"))
    .ok();
}

fn pick_random<T, R: IntoIterator<Item = T>>(from: &mut Vec<T>, refill: impl FnOnce() -> R) -> T {
    if from.is_empty() {
        from.extend(refill());
    }
    let i = rand::thread_rng().gen_range(0..from.len());
    from.remove(i)
}

fn print_crud_results(
    results: &BTreeMap<(FlowType, Box<[CrudOp<ResponseType>]>), bool>,
) -> serenity::utils::MessageBuilder {
    use std::fmt::Write;

    let mut mb = serenity::utils::MessageBuilder::new();
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

        write!(s, "\n{flow:<32} {ops:<62}  {}", success).unwrap();
    }

    tracing::info!("{s}");

    mb
}

#[async_trait::async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, int: interaction::Interaction) {
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
                (FlowType::TopLevel(int_ty), false) => {
                    try_pair(&int, &ctx.http, int_ty, untried, results).await
                },
                (FlowType::ModalSubmit(int_ty), false) => {
                    create_response(&int, &ctx.http, |res| {
                        sample_create_response(
                            int_ty,
                            interaction::InteractionResponseType::DeferredUpdateMessage,
                            res,
                        )
                    })
                    .await
                    .map_err(|e| tracing::warn!(%e))
                    .ok();
                },
                (FlowType::TopLevel(int_ty), true) => {
                    create_response(&int, &ctx.http, |res| {
                        sample_create_response(
                            int_ty,
                            interaction::InteractionResponseType::Modal,
                            res,
                        )
                    })
                    .await
                    .map_err(|e| tracing::error!(%e))
                    .ok();
                },
                (FlowType::ModalSubmit(int_ty), true) => {
                    try_pair(&int, &ctx.http, int_ty, untried, results).await
                },
            },
            TestMode::CrudBrute {
                ref to_try,
                ref mut untried,
                ref mut results,
            } => {
                let untried = untried.entry(flow).or_default();
                let ops = pick_random(untried, || to_try.iter().cloned());

                const TYPES: [ResponseType; 3] = [
                    ResponseType::Message,
                    ResponseType::UpdateMessage,
                    ResponseType::Modal,
                ];

                let mut types = vec![];
                let mut rem_types = vec![];
                let ops = ops
                    .into_iter()
                    .map(|op| {
                        let defer = matches!(op, CrudOp::Defer(_));
                        op.map(|i| {
                            loop {
                                if let Some(t) = types.get(i) {
                                    break *t;
                                }

                                types.push(loop {
                                    let ty = pick_random(&mut rem_types, || TYPES.iter().copied());

                                    if !defer || ty.defer().is_some() {
                                        break ty;
                                    }
                                });
                            }
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();

                let mut last_type = None;
                let mut res = Ok(());
                for (i, op) in ops.into_iter().enumerate() {
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
                            create_response(&int, &ctx.http, |res| {
                                sample_create_response(flow.initial_interaction(), ty.create(), res)
                            })
                            .await
                        },
                        CrudOp::Defer(_) => {
                            create_response(&int, &ctx.http, |res| {
                                sample_create_response(
                                    flow.initial_interaction(),
                                    ty.defer().unwrap(),
                                    res,
                                )
                            })
                            .await
                        },
                        CrudOp::Edit => {
                            edit_response(&int, &ctx.http, |res| sample_edit_response(res))
                                .await
                                .map(|_| ())
                        },
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

                    let mb = print_crud_results(results);

                    create_followup(&int, &ctx.http, |m| m.content(mb))
                        .await
                        .map_err(|e| tracing::warn!(%e, "Failed to send followup"))
                        .ok();
                }
                .instrument(span)
                .await
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
        .for_each(|p| match dotenv::from_filename(p) {
            Ok(_) => (),
            Err(dotenv::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => Err(e).unwrap(),
        });

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
                InteractionType::Command(command::CommandType::ChatInput),
                InteractionType::Command(command::CommandType::User),
                InteractionType::Command(command::CommandType::Message),
                InteractionType::MessageComponent,
            ];

            let res_types = [
                interaction::InteractionResponseType::ChannelMessageWithSource,
                interaction::InteractionResponseType::DeferredChannelMessageWithSource,
                interaction::InteractionResponseType::DeferredUpdateMessage,
                interaction::InteractionResponseType::UpdateMessage,
                interaction::InteractionResponseType::Modal,
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
            use CrudOp::*;

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
    };

    let mut client = Client::builder(discord_token, GatewayIntents::non_privileged())
        .event_handler(Handler { state: mode.into() })
        .await
        .unwrap();

    tokio::select! {
        s = tokio::signal::ctrl_c() => s.unwrap(),
        r = client.start() => r.unwrap(),
    }

    client.shard_manager.lock_owned().await.shutdown_all().await
}

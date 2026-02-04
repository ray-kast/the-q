use serenity::model::gateway::GatewayIntents;
use songbird::SerenityInit;

use crate::{prelude::*, util::DebugShim};

mod commands;
mod handler;

#[derive(Debug, clap::Args)]
pub struct ClientOpts {
    /// The Discord API token to use
    #[arg(long, env)]
    discord_token: DebugShim<String>,

    /// Connection string for RESP-compatible database
    #[arg(long, env)]
    redis_endpoint: String,

    #[command(flatten)]
    commands: commands::CommandOpts,
}

pub struct Client {
    serenity: serenity::Client,
}

pub async fn build(opts: ClientOpts) -> Result<Client> {
    let ClientOpts {
        discord_token,
        redis_endpoint,
        commands,
    } = opts;

    let redis = redis::Client::open(redis_endpoint)
        .context("Error connecting to RESP-compatible database")?;

    let intents = GatewayIntents::non_privileged(); // TODO
    let handler = handler::Handler::new_rc(handler::HandlerCx {
        opts: commands,
        redis,
    });

    let serenity = serenity::Client::builder(discord_token.0, intents)
        .event_handler_arc(handler)
        .register_songbird()
        .await
        .context("Error constructing Serenity client")?;

    Ok(Client { serenity })
}

impl marten::RunService for Client {
    type Output = ();

    async fn run(&mut self) -> Result<Self::Output> {
        self.serenity
            .start()
            .await
            .context("Fatal client error occurred")
            .and_then(|()| bail!("Client hung up unexpectedly"))
    }

    async fn stop(self) -> Result {
        self.serenity.shard_manager.shutdown_all().await;
        Ok(())
    }
}

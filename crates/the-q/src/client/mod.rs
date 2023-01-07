use serenity::{model::gateway::GatewayIntents, Client};
use songbird::SerenityInit;

use crate::{prelude::*, util::DebugShim};

mod command;
mod commands;
mod handler;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, clap::Args)]
pub struct ClientOpts {
    /// The Discord API token to use
    #[arg(long, env)]
    discord_token: DebugShim<String>,

    #[command(flatten)]
    commands: command::handler::Opts,
}

pub async fn build(opts: ClientOpts) -> Result<Client> {
    let ClientOpts {
        discord_token,
        commands,
    } = opts;

    let intents = GatewayIntents::non_privileged(); // TODO
    let handler = handler::Handler::new_rc(commands);

    Client::builder(discord_token.0, intents)
        .event_handler_arc(handler)
        .register_songbird()
        .await
        .context("Failed to construct Serenity client")
}

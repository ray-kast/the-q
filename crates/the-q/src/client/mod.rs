use serenity::{model::gateway::GatewayIntents, Client};
use songbird::SerenityInit;

use crate::{prelude::*, util::DebugShim};

mod commands;
mod handler;
mod interaction;

#[derive(Debug, clap::Args)]
pub struct ClientOpts {
    /// The Discord API token to use
    #[arg(long, env)]
    discord_token: DebugShim<String>,

    #[command(flatten)]
    interactions: interaction::handler::Opts,
}

pub async fn build(opts: ClientOpts) -> Result<Client> {
    let ClientOpts {
        discord_token,
        interactions,
    } = opts;

    let intents = GatewayIntents::non_privileged(); // TODO
    let handler = handler::Handler::new_rc(interactions);

    Client::builder(discord_token.0, intents)
        .event_handler_arc(handler)
        .register_songbird()
        .await
        .context("Error constructing Serenity client")
}

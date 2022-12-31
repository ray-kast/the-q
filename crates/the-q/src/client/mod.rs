use serenity::{model::gateway::GatewayIntents, Client};
use songbird::SerenityInit;

use crate::prelude::*;

mod handler;

pub async fn build(token: impl AsRef<str>) -> Result<Client> {
    let intents = GatewayIntents::non_privileged(); // TODO
    let handler = handler::Handler::new_rc();

    Client::builder(token, intents)
        .event_handler_arc(handler)
        .register_songbird()
        .await
        .context("Failed to construct Serenity client")
}

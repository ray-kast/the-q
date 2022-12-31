use serenity::{model::gateway::GatewayIntents, Client};

use crate::prelude::*;

mod handler;

pub async fn build(token: impl AsRef<str>) -> Result<Client> {
    let intents = GatewayIntents::empty();
    let handler = handler::Handler::new_rc();

    Client::builder(token, intents)
        .event_handler_arc(handler)
        .await
        .context("Failed to construct Serenity client")
}

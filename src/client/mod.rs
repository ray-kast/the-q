use std::sync::Arc;

use serenity::{model::gateway::GatewayIntents, Client};

use crate::prelude::*;

pub struct Handler;

#[async_trait]
impl serenity::client::EventHandler for Handler {}

pub async fn build(token: impl AsRef<str>) -> Result<Client> {
    let intents = GatewayIntents::empty();
    let framework = Arc::new(serenity::framework::StandardFramework::new());
    let handler = Arc::new(Handler);

    Client::builder(token, intents)
        .framework_arc(framework)
        .event_handler_arc(handler)
        .await
        .context("Failed to construct Serenity client")
}

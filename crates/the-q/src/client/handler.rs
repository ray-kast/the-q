use crate::prelude::*;

pub struct Handler;

impl Handler {
    pub fn new_rc() -> Arc<Self> { Arc::new(Self) }
}

#[async_trait]
impl serenity::client::EventHandler for Handler {}

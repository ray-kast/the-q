mod explode;
mod point;
mod say;
mod test;
mod vc;

pub(self) mod prelude {
    pub(super) use serenity::client::Context;

    pub use super::super::interaction::{
        command::{prelude::*, Args, CommandInfo},
        handler,
        handler::{
            CommandError, CommandHandler as Handler, CommandResponder, CommandResult, IntoErr,
        },
        response::{
            prelude::*, Component, Embed, Message, MessageBody, MessageOpts, Modal, ResponseData,
        },
        visitor,
        visitor::Visitor,
    };
    pub use crate::{
        prelude::*,
        proto::{component, modal},
    };
}

pub fn list() -> Vec<prelude::Arc<dyn prelude::Handler>> {
    use prelude::Arc;

    vec![
        Arc::new(explode::ExplodeCommand),
        Arc::new(point::PointCommand),
        Arc::new(say::SayCommand),
        Arc::new(test::TestCommand),
        Arc::new(vc::VcCommand),
    ]
}

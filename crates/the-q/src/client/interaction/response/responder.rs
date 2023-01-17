//! Response logic for gateway/webhook interactions
//!
//! # Notes
//!
//! After doing some fuzzing (see the `discord-pls` binary) I have determined
//! some rules regarding the flow of interaction responses:
//! - You **must** create **exactly one** response to acknowledge an
//!   interaction
//! - You **must** select one of the valid interaction response types according
//!   to the received interaction (see table below)
//! - Once an interaction is created, the referent of the edit and delete
//!   endpoints is:
//!   - if the response is `(DEFERRED_)CHANNEL_MESSAGE_WITH_SOURCE`, the newly
//!     created message
//!   - if the response is `(DEFERRED_)UPDATE_MESSAGE`, the existing edited
//!     message
//!   - else, nothing --- the edit and delete endpoints can only refer to a
//!     message
//! - You **may** edit the original response zero or more times
//! - You **may** delete the original response at most once
//! - You **must not** make _any_ interaction response calls after deleting the
//!   original response
//! - You **must not** create a followup message until the interaction is
//!   acknowledged
//! - You **may** create followup messages after the original response is
//!   deleted
//!
//! ## Valid Responses
//!
//! As stated above, valid interaction response types (listed below in the left
//! column) depend on the type of the received interaction (listed in the top
//! row).  Additionally, if an application receives interaction `A` and responds
//! to interaction `A` with type `MODAL`, upon receiving a `MODAL_SUBMIT`
//! interaction for this modal the valid response types will also depend on the
//! type of interaction `A` (the "initiating interaction"). See remarks for
//! details.
//!
//! |                                        | `APPLICATION_COMMAND` | `MESSAGE_COMPONENT` | `MODAL_SUBMIT` |
//! |---------------------------------------:|-----------------------|---------------------|----------------|
//! |          `CHANNEL_MESSAGE_WITH_SOURCE` | Yes                   | Yes                 | Yes            |
//! | `DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE` | Yes                   | Yes                 | Yes            |
//! |              `DEFERRED_UPDATE_MESSAGE` | **No**                | Yes                 | Yes\*          |
//! |                       `UPDATE_MESSAGE` | **No**                | Yes                 | Yes\*\*        |
//! |                                `MODAL` | Yes                   | Yes                 | **No**         |
//!
//! ### Remarks
//! _\* This works unconditionally, but the official documentation states it is
//! not a valid response if the initiating interaction was not of type
//! `MESSAGE_COMPONENT`_ \
//! _\*\* This only works if the initiating interaction was of type
//! `MESSAGE_COMPONENT`_
//!
//! ## Ping and Autocomplete
//!
//! As far as I could tell (and as far as I'm concerned) the only valid response
//! to `PING` and `APPLICATION_COMMAND_AUTOCOMPLETE` is a single create call (of
//! type `PONG` and `APPLICATION_COMMAND_AUTOCOMPLETE_RESULT`.  No other
//! response or followup calls should be made.  Additionally, as stated by the
//! docs, gateway clients do not need to handle `PING` interactions.

mod private {
    use serenity::{
        builder::{
            CreateInteractionResponse, CreateInteractionResponseFollowup, EditInteractionResponse,
        },
        http::Http,
        model::{
            application::interaction::{
                application_command::ApplicationCommandInteraction,
                message_component::MessageComponentInteraction, modal::ModalSubmitInteraction,
            },
            channel::Message,
            id::MessageId,
        },
    };

    use crate::prelude::*;

    // serenity why
    #[async_trait]
    pub trait Interaction: Sync {
        async fn create_response<'a>(
            &self,
            http: &Http,
            f: impl for<'b> FnOnce(
                &'b mut CreateInteractionResponse<'a>,
            ) -> &'b mut CreateInteractionResponse<'a>
            + Send,
        ) -> Result<(), serenity::Error>;

        async fn edit_response(
            &self,
            http: &Http,
            f: impl for<'a> FnOnce(&'a mut EditInteractionResponse) -> &'a mut EditInteractionResponse
            + Send,
        ) -> Result<Message, serenity::Error>;

        async fn delete_response(&self, http: &Http) -> Result<(), serenity::Error>;

        async fn create_followup_message<'a>(
            &self,
            http: &Http,
            f: impl for<'b> FnOnce(
                &'b mut CreateInteractionResponseFollowup<'a>,
            ) -> &'b mut CreateInteractionResponseFollowup<'a>
            + Send,
        ) -> Result<Message, serenity::Error>;

        async fn edit_followup_message<'a>(
            &self,
            http: &Http,
            id: MessageId,
            f: impl for<'b> FnOnce(
                &'b mut CreateInteractionResponseFollowup<'a>,
            ) -> &'b mut CreateInteractionResponseFollowup<'a>
            + Send,
        ) -> Result<Message, serenity::Error>;

        async fn delete_followup_message(
            &self,
            http: &Http,
            id: MessageId,
        ) -> Result<(), serenity::Error>;
    }

    macro_rules! interaction {
        ($ty:ident) => {
            #[async_trait]
            impl Interaction for $ty {
                #[inline]
                async fn create_response<'a>(
                    &self,
                    http: &Http,
                    f: impl for<'b> FnOnce(
                        &'b mut CreateInteractionResponse<'a>,
                    ) -> &'b mut CreateInteractionResponse<'a>
                    + Send,
                ) -> Result<(), serenity::Error> {
                    $ty::create_interaction_response(self, http, f).await
                }

                #[inline]
                async fn edit_response(
                    &self,
                    http: &Http,
                    f: impl for<'a> FnOnce(
                        &'a mut EditInteractionResponse,
                    ) -> &'a mut EditInteractionResponse
                    + Send,
                ) -> Result<Message, serenity::Error> {
                    $ty::edit_original_interaction_response(self, http, f).await
                }

                #[inline]
                async fn delete_response(&self, http: &Http) -> Result<(), serenity::Error> {
                    $ty::delete_original_interaction_response(self, http).await
                }

                #[inline]
                async fn create_followup_message<'a>(
                    &self,
                    http: &Http,
                    f: impl for<'b> FnOnce(
                        &'b mut CreateInteractionResponseFollowup<'a>,
                    ) -> &'b mut CreateInteractionResponseFollowup<'a>
                    + Send,
                ) -> Result<Message, serenity::Error> {
                    $ty::create_followup_message(self, http, f).await
                }

                #[inline]
                async fn edit_followup_message<'a>(
                    &self,
                    http: &Http,
                    id: MessageId,
                    f: impl for<'b> FnOnce(
                        &'b mut CreateInteractionResponseFollowup<'a>,
                    ) -> &'b mut CreateInteractionResponseFollowup<'a>
                    + Send,
                ) -> Result<Message, serenity::Error> {
                    $ty::edit_followup_message(self, http, id, f).await
                }

                #[inline]
                async fn delete_followup_message(
                    &self,
                    http: &Http,
                    id: MessageId,
                ) -> Result<(), serenity::Error> {
                    $ty::delete_followup_message(self, http, id).await
                }
            }
        };
    }

    interaction!(ApplicationCommandInteraction);
    interaction!(MessageComponentInteraction);
    interaction!(ModalSubmitInteraction);

    #[derive(Debug)]
    pub struct ResponderCore<'a, I> {
        pub(super) http: &'a Http,
        pub(super) int: &'a I,
    }

    impl<'a, I> Clone for ResponderCore<'a, I> {
        fn clone(&self) -> Self { *self }
    }
    impl<'a, I> Copy for ResponderCore<'a, I> {}

    pub trait Responder {
        type Interaction: Interaction;

        fn core(&self) -> ResponderCore<'_, Self::Interaction>;
    }

    impl<'a, I: Interaction> Responder for super::InitResponder<'a, I> {
        type Interaction = I;

        #[inline]
        fn core(&self) -> ResponderCore<'_, Self::Interaction> { self.0 }
    }

    impl<'a, I: Interaction> Responder for super::CreatedResponder<'a, I> {
        type Interaction = I;

        #[inline]
        fn core(&self) -> ResponderCore<'_, Self::Interaction> { self.0 }
    }

    impl<'a, I: Interaction> Responder for super::VoidResponder<'a, I> {
        type Interaction = I;

        #[inline]
        fn core(&self) -> ResponderCore<'_, Self::Interaction> { self.0 }
    }

    pub trait CreateUpdate: Interaction {}
    impl CreateUpdate for MessageComponentInteraction {}
    impl CreateUpdate for ModalSubmitInteraction {}

    pub trait CreateModal: Interaction {}
    impl CreateModal for ApplicationCommandInteraction {}
    impl CreateModal for MessageComponentInteraction {}

    pub trait CreateFollowup {}
    impl<'a, I> CreateFollowup for super::CreatedResponder<'a, I> {}
    impl<'a, I> CreateFollowup for super::VoidResponder<'a, I> {}
}

use private::{Interaction, ResponderCore};
use serenity::{http::Http, model::application::interaction::InteractionResponseType};

use super::{Message, MessageBody, MessageOpts, Modal, ResponseData};
use crate::prelude::*;

#[repr(transparent)]
pub struct Followup(serenity::model::channel::Message);

#[async_trait]
pub trait ResponderExt: private::Responder {
    #[inline]
    async fn create_followup(&self, msg: Message) -> Result<Followup, serenity::Error>
    where Self: private::CreateFollowup {
        let ResponderCore { http, int } = self.core();
        int.create_followup_message(http, |f| msg.build_followup(f))
            .await
            .map(Followup)
    }

    #[inline]
    async fn edit_followup(&self, fup: &mut Followup, msg: Message) -> Result<(), serenity::Error>
    where Self: private::CreateFollowup {
        let ResponderCore { http, int } = self.core();
        *fup = Followup(
            int.edit_followup_message(http, fup.0.id, |f| msg.build_followup(f))
                .await?,
        );

        Ok(())
    }

    #[inline]
    async fn delete_followup(&self, fup: Followup) -> Result<(), serenity::Error>
    where Self: private::CreateFollowup {
        let ResponderCore { http, int } = self.core();
        int.delete_followup_message(http, fup.0.id).await
    }
}

impl<R: private::Responder> ResponderExt for R {}

#[derive(Debug)]
#[repr(transparent)]
pub struct InitResponder<'a, I>(ResponderCore<'a, I>);

impl<'a, I> InitResponder<'a, I> {
    #[inline]
    #[must_use]
    pub fn new(http: &'a Http, int: &'a I) -> Self { Self(ResponderCore { http, int }) }

    #[inline]
    #[must_use]
    pub fn void(self) -> VoidResponder<'a, I> { VoidResponder(self.0) }
}

impl<'a, I: private::Interaction> InitResponder<'a, I> {
    #[inline]
    async fn create<T>(
        self,
        ty: InteractionResponseType,
        data: impl ResponseData + Send,
        next: impl FnOnce(ResponderCore<'a, I>) -> T,
    ) -> Result<T, serenity::Error> {
        let Self(core @ ResponderCore { http, int }) = self;
        int.create_response(http, |res| {
            res.kind(ty)
                .interaction_response_data(|d| data.build_response_data(d))
        })
        .await?;
        Ok(next(core))
    }

    #[inline]
    pub async fn create_message(
        self,
        msg: Message,
    ) -> Result<CreatedResponder<'a, I>, serenity::Error> {
        self.create(
            InteractionResponseType::ChannelMessageWithSource,
            msg,
            CreatedResponder,
        )
        .await
    }

    #[inline]
    pub async fn defer_message(
        self,
        opts: MessageOpts,
    ) -> Result<CreatedResponder<'a, I>, serenity::Error> {
        self.create(
            InteractionResponseType::DeferredChannelMessageWithSource,
            opts,
            CreatedResponder,
        )
        .await
    }
}

impl<'a, I: private::CreateUpdate> InitResponder<'a, I> {
    #[inline]
    pub async fn update_message(
        self,
        msg: Message, // TODO: is opts necessary?
    ) -> Result<CreatedResponder<'a, I>, serenity::Error> {
        self.create(
            InteractionResponseType::UpdateMessage,
            msg,
            CreatedResponder,
        )
        .await
    }

    #[inline]
    pub async fn defer_update(
        self,
        opts: MessageOpts, // TODO: is this usable?
    ) -> Result<CreatedResponder<'a, I>, serenity::Error> {
        self.create(
            InteractionResponseType::DeferredUpdateMessage,
            opts,
            CreatedResponder,
        )
        .await
    }
}

impl<'a, I: private::CreateModal> InitResponder<'a, I> {
    #[inline]
    pub async fn modal(self, modal: Modal) -> Result<VoidResponder<'a, I>, serenity::Error> {
        self.create(InteractionResponseType::Modal, modal, VoidResponder)
            .await
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CreatedResponder<'a, I>(ResponderCore<'a, I>);

impl<'a, I: private::Interaction> CreatedResponder<'a, I> {
    #[inline]
    #[must_use]
    pub fn void(self) -> VoidResponder<'a, I> { VoidResponder(self.0) }

    #[inline]
    pub async fn edit(
        &self,
        res: MessageBody,
    ) -> Result<serenity::model::channel::Message, serenity::Error> {
        self.0
            .int
            .edit_response(self.0.http, |e| res.build_edit_response(e))
            .await
    }

    #[inline]
    pub async fn delete(self) -> Result<(), serenity::Error> {
        self.0.int.delete_response(self.0.http).await
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct VoidResponder<'a, I>(ResponderCore<'a, I>);

#[derive(Debug)]
pub enum AckedResponder<'a, I> {
    Created(CreatedResponder<'a, I>),
    Void(VoidResponder<'a, I>),
}

impl<'a, I> From<CreatedResponder<'a, I>> for AckedResponder<'a, I> {
    #[inline]
    fn from(val: CreatedResponder<'a, I>) -> Self { Self::Created(val) }
}

impl<'a, I> From<VoidResponder<'a, I>> for AckedResponder<'a, I> {
    #[inline]
    fn from(val: VoidResponder<'a, I>) -> Self { Self::Void(val) }
}

pub enum BorrowedResponder<'a, I> {
    Init(InitResponder<'a, I>),
    Void(VoidResponder<'a, I>),
    Poison,
}

impl<'a, I> BorrowedResponder<'a, I> {
    #[inline]
    #[must_use]
    pub fn new(http: &'a Http, int: &'a I) -> Self {
        Self::Init(InitResponder(ResponderCore { http, int }))
    }
}

impl<'a, I: private::Interaction> BorrowedResponder<'a, I> {
    pub async fn upsert_message(
        &mut self,
        msg: Message,
    ) -> Result<Option<Followup>, serenity::Error> {
        match self {
            Self::Init(_) => {
                let Self::Init(resp) = mem::replace(self, Self::Poison) else {
                    unreachable!()
                };

                let resp = resp.create_message(msg).await?;
                *self = Self::Void(resp.void());

                Ok(None)
            },
            Self::Void(v) => v.create_followup(msg).await.map(Some),
            Self::Poison => panic!("Attempt to use poisoned responder"),
        }
    }
}

pub struct BorrowingResponder<'a, 'b, I>(&'a mut BorrowedResponder<'b, I>);

impl<'a, 'b, I> BorrowingResponder<'a, 'b, I> {
    #[inline]
    #[must_use]
    pub fn new(resp: &'a mut BorrowedResponder<'b, I>) -> Self {
        assert!(
            matches!(resp, BorrowedResponder::Init(_)),
            "BorrowingResponder::new called with a non-Init responder",
        );

        Self(resp)
    }

    fn take(self) -> InitResponder<'b, I> {
        // TODO: consider poisoning self.0 for the duration of the response operation
        let BorrowedResponder::Init(InitResponder(core)) = *self.0 else {
            unreachable!();
        };

        match mem::replace(self.0, BorrowedResponder::Void(VoidResponder(core))) {
            BorrowedResponder::Init(i) => i,
            _ => unreachable!(),
        }
    }
}

impl<'a, 'b, I: private::Interaction> BorrowingResponder<'a, 'b, I> {
    #[inline]
    pub async fn create_message(
        self,
        msg: Message,
    ) -> Result<CreatedResponder<'b, I>, serenity::Error> {
        self.take().create_message(msg).await
    }

    #[inline]
    pub async fn defer_message(
        self,
        opts: MessageOpts,
    ) -> Result<CreatedResponder<'b, I>, serenity::Error> {
        self.take().defer_message(opts).await
    }
}

impl<'a, 'b, I: private::CreateUpdate> BorrowingResponder<'a, 'b, I> {
    #[inline]
    pub async fn update_message(
        self,
        msg: Message,
    ) -> Result<CreatedResponder<'b, I>, serenity::Error> {
        self.take().update_message(msg).await
    }

    #[inline]
    pub async fn defer_update(
        self,
        opts: MessageOpts, // TODO: is this usable?
    ) -> Result<CreatedResponder<'b, I>, serenity::Error> {
        self.take().defer_update(opts).await
    }
}

impl<'a, 'b, I: private::CreateModal> BorrowingResponder<'a, 'b, I> {
    #[inline]
    pub async fn modal(self, modal: Modal) -> Result<VoidResponder<'b, I>, serenity::Error> {
        self.take().modal(modal).await
    }
}

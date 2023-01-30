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

    use super::super::modal;
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
    pub struct ResponderCore<'a, S, I> {
        pub(super) http: &'a Http,
        pub(super) int: &'a I,
        pub(super) schema: PhantomData<fn(S)>,
    }

    impl<'a, S, I> Clone for ResponderCore<'a, S, I> {
        fn clone(&self) -> Self { *self }
    }
    impl<'a, S, I> Copy for ResponderCore<'a, S, I> {}

    pub trait Responder {
        type Schema: super::Schema;
        type Interaction: Interaction;

        fn core(&self) -> ResponderCore<'_, Self::Schema, Self::Interaction>;
    }

    impl<'a, S: super::Schema, I: Interaction> Responder for super::InitResponder<'a, S, I> {
        type Interaction = I;
        type Schema = S;

        #[inline]
        fn core(&self) -> ResponderCore<'_, S, I> { self.0 }
    }

    impl<'a, S: super::Schema, I: Interaction> Responder for super::CreatedResponder<'a, S, I> {
        type Interaction = I;
        type Schema = S;

        #[inline]
        fn core(&self) -> ResponderCore<'_, S, I> { self.0 }
    }

    impl<'a, S: super::Schema, I: Interaction> Responder for super::VoidResponder<'a, S, I> {
        type Interaction = I;
        type Schema = S;

        #[inline]
        fn core(&self) -> ResponderCore<'_, S, I> { self.0 }
    }

    pub trait CreateUpdate: Interaction {}
    impl CreateUpdate for MessageComponentInteraction {}
    impl CreateUpdate for ModalSubmitInteraction {}

    pub trait CreateModal: Interaction {
        const MODAL_SOURCE: modal::ModalSource;
    }
    impl CreateModal for ApplicationCommandInteraction {
        const MODAL_SOURCE: modal::ModalSource = modal::ModalSource::Command;
    }
    impl CreateModal for MessageComponentInteraction {
        const MODAL_SOURCE: modal::ModalSource = modal::ModalSource::Component;
    }

    pub trait CreateFollowup {}
    impl<'a, S, I> CreateFollowup for super::CreatedResponder<'a, S, I> {}
    impl<'a, S, I> CreateFollowup for super::VoidResponder<'a, S, I> {}
}

use private::{Interaction, ResponderCore};
use serenity::{http::Http, model::application::interaction::InteractionResponseType};

use super::{
    super::rpc::Schema, id, Message, MessageBody, MessageOpts, Modal, ModalSourceHandle,
    ResponseData,
};
use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum ResponseError {
    #[error("Serenity error")]
    Serenity(#[from] serenity::Error),
    #[error("Custom ID error for component or modal")]
    Id(#[from] id::Error),
}

#[repr(transparent)]
pub struct Followup(serenity::model::channel::Message);

#[async_trait]
pub trait ResponderExt<S: Schema>: private::Responder {
    #[inline]
    async fn create_followup(
        &self,
        msg: Message<'_, S::Component, id::Error>,
    ) -> Result<Followup, ResponseError>
    where
        Self: private::CreateFollowup,
        S::Component: 'async_trait,
    {
        let msg = msg.prepare()?;
        let ResponderCore {
            http,
            int,
            schema: _,
        } = self.core();
        Ok(int
            .create_followup_message(http, |f| msg.build_followup(f))
            .await
            .map(Followup)?)
    }

    #[inline]
    async fn edit_followup(
        &self,
        fup: &mut Followup,
        msg: Message<'_, S::Component, id::Error>,
    ) -> Result<(), ResponseError>
    where
        Self: private::CreateFollowup,
        S::Component: 'async_trait,
    {
        let msg = msg.prepare()?;
        let ResponderCore {
            http,
            int,
            schema: _,
        } = self.core();
        *fup = Followup(
            int.edit_followup_message(http, fup.0.id, |f| msg.build_followup(f))
                .await?,
        );

        Ok(())
    }

    #[inline]
    async fn delete_followup(&self, fup: Followup) -> Result<(), serenity::Error>
    where Self: private::CreateFollowup {
        let ResponderCore {
            http,
            int,
            schema: _,
        } = self.core();
        int.delete_followup_message(http, fup.0.id).await
    }
}

impl<R: private::Responder> ResponderExt<R::Schema> for R {}

#[derive(Debug)]
#[repr(transparent)]
pub struct InitResponder<'a, S, I>(ResponderCore<'a, S, I>);

impl<'a, S, I> InitResponder<'a, S, I> {
    #[inline]
    #[must_use]
    pub fn new(http: &'a Http, int: &'a I) -> Self {
        Self(ResponderCore {
            http,
            int,
            schema: PhantomData::default(),
        })
    }

    #[inline]
    #[must_use]
    pub fn void(self) -> VoidResponder<'a, S, I> { VoidResponder(self.0) }
}

impl<'a, S: Schema, I: private::Interaction> InitResponder<'a, S, I> {
    #[inline]
    async fn create<T>(
        self,
        ty: InteractionResponseType,
        data: impl ResponseData<'_> + Send,
        next: impl FnOnce(ResponderCore<'a, S, I>) -> T,
    ) -> Result<T, serenity::Error> {
        let Self(
            core @ ResponderCore {
                http,
                int,
                schema: _,
            },
        ) = self;
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
        msg: Message<'_, S::Component, id::Error>,
    ) -> Result<CreatedResponder<'a, S, I>, ResponseError> {
        let msg = msg.prepare()?;
        Ok(self
            .create(
                InteractionResponseType::ChannelMessageWithSource,
                msg,
                CreatedResponder,
            )
            .await?)
    }

    #[inline]
    pub async fn defer_message(
        self,
        opts: MessageOpts,
    ) -> Result<CreatedResponder<'a, S, I>, serenity::Error> {
        self.create(
            InteractionResponseType::DeferredChannelMessageWithSource,
            opts,
            CreatedResponder,
        )
        .await
    }
}

impl<'a, S: Schema, I: private::CreateUpdate> InitResponder<'a, S, I> {
    #[inline]
    pub async fn update_message(
        self,
        msg: Message<'_, S::Component, id::Error>, // TODO: is opts necessary?
    ) -> Result<CreatedResponder<'a, S, I>, ResponseError> {
        let msg = msg.prepare()?;
        Ok(self
            .create(
                InteractionResponseType::UpdateMessage,
                msg,
                CreatedResponder,
            )
            .await?)
    }

    #[inline]
    pub async fn defer_update(
        self,
        opts: MessageOpts, // TODO: is this usable?
    ) -> Result<CreatedResponder<'a, S, I>, serenity::Error> {
        self.create(
            InteractionResponseType::DeferredUpdateMessage,
            opts,
            CreatedResponder,
        )
        .await
    }
}

impl<'a, S: Schema, I: private::CreateModal> InitResponder<'a, S, I> {
    #[inline]
    pub async fn modal(
        self,
        modal: impl FnOnce(ModalSourceHandle) -> Modal<S, id::Error>,
    ) -> Result<VoidResponder<'a, S, I>, ResponseError> {
        let modal = modal(ModalSourceHandle(I::MODAL_SOURCE)).prepare()?;
        Ok(self
            .create(InteractionResponseType::Modal, modal, VoidResponder)
            .await?)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CreatedResponder<'a, S, I>(ResponderCore<'a, S, I>);

impl<'a, S: Schema, I: private::Interaction> CreatedResponder<'a, S, I> {
    #[inline]
    #[must_use]
    pub fn void(self) -> VoidResponder<'a, S, I> { VoidResponder(self.0) }

    #[inline]
    pub async fn edit(
        &self,
        res: MessageBody<S::Component, id::Error>,
    ) -> Result<serenity::model::channel::Message, ResponseError> {
        let res = res.prepare()?;
        Ok(self
            .0
            .int
            .edit_response(self.0.http, |e| res.build_edit_response(e))
            .await?)
    }

    #[inline]
    pub async fn delete(self) -> Result<(), serenity::Error> {
        self.0.int.delete_response(self.0.http).await
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct VoidResponder<'a, S, I>(ResponderCore<'a, S, I>);

#[derive(Debug)]
pub enum AckedResponder<'a, S, I> {
    Created(CreatedResponder<'a, S, I>),
    Void(VoidResponder<'a, S, I>),
}

impl<'a, S, I> From<CreatedResponder<'a, S, I>> for AckedResponder<'a, S, I> {
    #[inline]
    fn from(val: CreatedResponder<'a, S, I>) -> Self { Self::Created(val) }
}

impl<'a, S, I> From<VoidResponder<'a, S, I>> for AckedResponder<'a, S, I> {
    #[inline]
    fn from(val: VoidResponder<'a, S, I>) -> Self { Self::Void(val) }
}

pub enum BorrowedResponder<'a, S, I> {
    Init(InitResponder<'a, S, I>),
    Void(VoidResponder<'a, S, I>),
    Poison,
}

impl<'a, S, I> BorrowedResponder<'a, S, I> {
    #[inline]
    #[must_use]
    pub fn new(http: &'a Http, int: &'a I) -> Self {
        Self::Init(InitResponder(ResponderCore {
            http,
            int,
            schema: PhantomData::default(),
        }))
    }
}

impl<'a, S: Schema, I: private::Interaction> BorrowedResponder<'a, S, I> {
    pub async fn upsert_message(
        &mut self,
        msg: Message<'_, S::Component, id::Error>,
    ) -> Result<Option<Followup>, ResponseError> {
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

pub struct BorrowingResponder<'a, 'b, S, I>(&'a mut BorrowedResponder<'b, S, I>);

impl<'a, 'b, S, I> BorrowingResponder<'a, 'b, S, I> {
    #[inline]
    #[must_use]
    pub fn new(resp: &'a mut BorrowedResponder<'b, S, I>) -> Self {
        assert!(
            matches!(resp, BorrowedResponder::Init(_)),
            "BorrowingResponder::new called with a non-Init responder",
        );

        Self(resp)
    }

    /// # Safety
    /// This function should only be called with a closure that invokes one of
    /// the create response endpoints, otherwise the state update behavior is
    /// incorrect.
    async unsafe fn take<F: Future<Output = Result<T, E>>, T, E>(
        self,
        f: impl FnOnce(InitResponder<'b, S, I>) -> F,
    ) -> Result<T, E> {
        let BorrowedResponder::Init(init) = mem::replace(self.0, BorrowedResponder::Poison) else {
            unreachable!();
        };

        let core = init.0;
        let res = f(init).await;

        *self.0 = match res {
            Ok(_) => BorrowedResponder::Void(VoidResponder(core)),
            Err(_) => BorrowedResponder::Init(InitResponder(core)),
        };

        res
    }
}

impl<'a, 'b, S: Schema, I: private::Interaction> BorrowingResponder<'a, 'b, S, I> {
    #[inline]
    pub async fn create_message(
        self,
        msg: Message<'_, S::Component, id::Error>,
    ) -> Result<CreatedResponder<'b, S, I>, ResponseError> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.create_message(msg)).await }
    }

    #[inline]
    pub async fn defer_message(
        self,
        opts: MessageOpts,
    ) -> Result<CreatedResponder<'b, S, I>, serenity::Error> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.defer_message(opts)).await }
    }
}

impl<'a, 'b, S: Schema, I: private::CreateUpdate> BorrowingResponder<'a, 'b, S, I> {
    #[inline]
    pub async fn update_message(
        self,
        msg: Message<'_, S::Component, id::Error>,
    ) -> Result<CreatedResponder<'b, S, I>, ResponseError> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.update_message(msg)).await }
    }

    #[inline]
    pub async fn defer_update(
        self,
        opts: MessageOpts, // TODO: is this usable?
    ) -> Result<CreatedResponder<'b, S, I>, serenity::Error> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.defer_update(opts)).await }
    }
}

impl<'a, 'b, S: Schema, I: private::CreateModal> BorrowingResponder<'a, 'b, S, I> {
    #[inline]
    pub async fn modal(
        self,
        f: impl FnOnce(ModalSourceHandle) -> Modal<S, id::Error>,
    ) -> Result<VoidResponder<'b, S, I>, ResponseError> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.modal(f)).await }
    }
}

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
    use std::marker::PhantomData;

    use serenity::{
        builder::{
            CreateInteractionResponse, CreateInteractionResponseFollowup, EditInteractionResponse,
        },
        http::Http,
        model::{
            application::{CommandInteraction, ComponentInteraction, ModalInteraction},
            channel::Message,
            id::MessageId,
        },
    };

    use super::super::modal;

    // serenity why
    #[async_trait::async_trait]
    pub trait Interaction: Sync {
        async fn create_response(
            &self,
            http: &Http,
            res: CreateInteractionResponse,
        ) -> Result<(), serenity::Error>;

        async fn edit_response(
            &self,
            http: &Http,
            res: EditInteractionResponse,
        ) -> Result<Message, serenity::Error>;

        async fn delete_response(&self, http: &Http) -> Result<(), serenity::Error>;

        async fn create_followup_message(
            &self,
            http: &Http,
            fup: CreateInteractionResponseFollowup,
        ) -> Result<Message, serenity::Error>;

        async fn edit_followup_message(
            &self,
            http: &Http,
            id: MessageId,
            fup: CreateInteractionResponseFollowup,
        ) -> Result<Message, serenity::Error>;

        async fn delete_followup_message(
            &self,
            http: &Http,
            id: MessageId,
        ) -> Result<(), serenity::Error>;
    }

    macro_rules! interaction {
        ($ty:ident) => {
            #[async_trait::async_trait]
            impl Interaction for $ty {
                #[inline]
                async fn create_response(
                    &self,
                    http: &Http,
                    res: CreateInteractionResponse,
                ) -> Result<(), serenity::Error> {
                    $ty::create_response(self, http, res).await
                }

                #[inline]
                async fn edit_response(
                    &self,
                    http: &Http,
                    res: EditInteractionResponse,
                ) -> Result<Message, serenity::Error> {
                    $ty::edit_response(self, http, res).await
                }

                #[inline]
                async fn delete_response(&self, http: &Http) -> Result<(), serenity::Error> {
                    $ty::delete_response(self, http).await
                }

                #[inline]
                async fn create_followup_message(
                    &self,
                    http: &Http,
                    fup: CreateInteractionResponseFollowup,
                ) -> Result<Message, serenity::Error> {
                    $ty::create_followup_message(self, http, fup).await
                }

                #[inline]
                async fn edit_followup_message(
                    &self,
                    http: &Http,
                    id: MessageId,
                    fup: CreateInteractionResponseFollowup,
                ) -> Result<Message, serenity::Error> {
                    $ty::edit_followup_message(self, http, id, fup).await
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

    interaction!(CommandInteraction);
    interaction!(ComponentInteraction);
    interaction!(ModalInteraction);

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
    impl CreateUpdate for ComponentInteraction {}

    pub trait TryCreateUpdate: Interaction {}
    impl TryCreateUpdate for ModalInteraction {}

    pub trait CreateModal: Interaction {
        const MODAL_SOURCE: modal::ModalSource;
    }
    impl CreateModal for CommandInteraction {
        const MODAL_SOURCE: modal::ModalSource = modal::ModalSource::Command;
    }
    impl CreateModal for ComponentInteraction {
        const MODAL_SOURCE: modal::ModalSource = modal::ModalSource::Component;
    }

    pub trait CreateFollowup {}
    impl<'a, S, I> CreateFollowup for super::CreatedResponder<'a, S, I> {}
    impl<'a, S, I> CreateFollowup for super::VoidResponder<'a, S, I> {}
}

use std::{future::Future, marker::PhantomData, mem};

use private::{Interaction, ResponderCore};
use qcore::build_with::BuildDefault;
use serenity::{builder::CreateInteractionResponse, http::Http};

use super::{
    super::rpc::Schema, id, Message, MessageBody, MessageOpts, Modal, ModalSourceHandle, Prepare,
};

/// An error arising from sending an interaction response
#[derive(Debug, thiserror::Error)]
pub enum ResponseError {
    /// A [`serenity`] (or Discord) error occurred
    #[error("Serenity error")]
    Serenity(#[from] serenity::Error),
    /// An error occurred transcoding an [`Id`](id::Id)
    #[error("Custom ID error for component or modal")]
    Id(#[from] id::Error),
}

/// A followup message returned from a responder
#[derive(Debug)]
#[repr(transparent)]
pub struct Followup(serenity::model::channel::Message);

/// Common methods for all responder types
#[async_trait::async_trait]
pub trait ResponderExt<S: Schema>: private::Responder {
    /// Create a followup message for this interaction
    #[inline]
    async fn create_followup(
        &self,
        msg: Message<S::Component, id::Error>,
    ) -> Result<Followup, ResponseError>
    where
        Self: private::CreateFollowup,
        S::Component: 'async_trait,
    {
        let ResponderCore {
            http,
            int,
            schema: _,
        } = self.core();
        Ok(int
            .create_followup_message(http, msg.prepare()?.build_default())
            .await
            .map(Followup)?)
    }

    /// Edit the given followup message for this interaction
    #[inline]
    async fn edit_followup(
        &self,
        fup: &mut Followup,
        msg: Message<S::Component>,
    ) -> Result<(), ResponseError>
    where
        Self: private::CreateFollowup,
        S::Component: 'async_trait,
    {
        let ResponderCore {
            http,
            int,
            schema: _,
        } = self.core();
        *fup = Followup(
            int.edit_followup_message(http, fup.0.id, msg.build_default())
                .await?,
        );

        Ok(())
    }

    /// Delete the given followup message for this interaction
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

/// A responder in its initial state
///
/// In this state, a response must be created before any other operations may
/// occur.
#[derive(Debug)]
#[repr(transparent)]
pub struct InitResponder<'a, S, I>(ResponderCore<'a, S, I>);

impl<'a, S, I> InitResponder<'a, S, I> {
    /// Wrap an HTTP client and interaction reference in a new responder
    #[inline]
    #[must_use]
    pub fn new(http: &'a Http, int: &'a I) -> Self {
        Self(ResponderCore {
            http,
            int,
            schema: PhantomData,
        })
    }
}

impl<'a, S: Schema, I: private::Interaction> InitResponder<'a, S, I> {
    #[inline]
    async fn create<T>(
        self,
        res: impl Into<CreateInteractionResponse> + Send,
        next: impl FnOnce(ResponderCore<'a, S, I>) -> T,
    ) -> Result<T, serenity::Error> {
        let Self(
            core @ ResponderCore {
                http,
                int,
                schema: _,
            },
        ) = self;
        int.create_response(http, res.into()).await?;
        Ok(next(core))
    }

    /// Create a channel message response
    ///
    /// # Errors
    /// This method returns an error if the message contains errors or an API
    /// error is received.
    #[inline]
    pub async fn create_message(
        self,
        msg: Message<S::Component, id::Error>,
    ) -> Result<CreatedResponder<'a, S, I>, ResponseError> {
        Ok(self
            .create(
                CreateInteractionResponse::Message(msg.prepare()?.build_default()),
                CreatedResponder,
            )
            .await?)
    }

    /// Create a deferred channel message response
    ///
    /// # Errors
    /// This method returns an error if an API error is received.
    #[inline]
    pub async fn defer_message(
        self,
        // TODO: this is a message field now, can we send messages?
        opts: MessageOpts,
    ) -> Result<CreatedResponder<'a, S, I>, serenity::Error> {
        self.create(
            CreateInteractionResponse::Defer(opts.build_default()),
            CreatedResponder,
        )
        .await
    }
}

impl<'a, S: Schema, I: private::CreateUpdate> InitResponder<'a, S, I> {
    /// Create a message update response
    ///
    /// # Errors
    /// This method returns an error if the message contains errors or an API
    /// error is received.
    #[inline]
    pub async fn update_message(
        self,
        msg: Message<S::Component, id::Error>, // TODO: is opts necessary?
    ) -> Result<CreatedResponder<'a, S, I>, ResponseError> {
        Ok(self
            .create(
                CreateInteractionResponse::UpdateMessage(msg.prepare()?.build_default()),
                CreatedResponder,
            )
            .await?)
    }

    /// Create a deferred message update response
    ///
    /// # Errors
    /// This method returns an error if an API error is received.
    #[inline]
    pub async fn defer_update(self) -> Result<CreatedResponder<'a, S, I>, serenity::Error> {
        self.create(CreateInteractionResponse::Acknowledge, CreatedResponder)
            .await
    }
}

impl<'a, S: Schema, I: private::TryCreateUpdate> InitResponder<'a, S, I> {}

impl<'a, S: Schema, I: private::CreateModal> InitResponder<'a, S, I> {
    /// Create a modal dialog response
    ///
    /// # Errors
    /// This method returns an error if the modal contains errors or an API
    /// error is received.
    #[inline]
    pub async fn modal(
        self,
        modal: impl FnOnce(ModalSourceHandle) -> Modal<S, id::Error>,
    ) -> Result<VoidResponder<'a, S, I>, ResponseError> {
        let modal = modal(ModalSourceHandle(I::MODAL_SOURCE)).prepare()?;
        Ok(self
            .create(
                CreateInteractionResponse::Modal(modal.into()),
                VoidResponder,
            )
            .await?)
    }
}

/// A responder in its post-create state
///
/// In this state, a response message has been created and it may be edited zero
/// or more times or deleted once.
#[derive(Debug)]
#[repr(transparent)]
pub struct CreatedResponder<'a, S, I>(ResponderCore<'a, S, I>);

impl<'a, S: Schema, I: private::Interaction> CreatedResponder<'a, S, I> {
    /// Void this responder, disallowing any response methods from being called
    #[inline]
    #[must_use]
    pub fn void(self) -> VoidResponder<'a, S, I> { VoidResponder(self.0) }

    /// Edit the interaction response message
    ///
    /// # Errors
    /// This method returns an error if the message contains errors or an API
    /// error is received.
    #[inline]
    pub async fn edit(
        &self,
        res: MessageBody<S::Component, id::Error>,
    ) -> Result<serenity::model::channel::Message, ResponseError> {
        Ok(self
            .0
            .int
            .edit_response(self.0.http, res.prepare()?.build_default())
            .await?)
    }

    /// Delete the interaction response message
    ///
    /// # Errors
    /// This method returns an error if an API error is received.
    #[inline]
    pub async fn delete(self) -> Result<(), serenity::Error> {
        self.0.int.delete_response(self.0.http).await
    }
}

/// A responder in its "voided" state
///
/// In this state, the response message has been deleted, the response was not
/// an editable message, or the responder was voided.  No additional response
/// actions may be performed.
#[derive(Debug)]
#[repr(transparent)]
pub struct VoidResponder<'a, S, I>(ResponderCore<'a, S, I>);

/// An "acknowledged" responder
///
/// This wrapper holds a responder that is guaranteed to have created a
/// response, but for which an editable response message may or may not exist.
#[derive(Debug)]
pub enum AckedResponder<'a, S, I> {
    /// A responder created an editable message
    Created(CreatedResponder<'a, S, I>),
    /// A responder did not create a message or was voided
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

/// A responder that can be borrowed and mutated by a [`BorrowingResponder`]
#[derive(Debug)]
pub enum BorrowedResponder<'a, S, I> {
    /// An initial-state responder
    Init(InitResponder<'a, S, I>),
    /// A voided responder
    Void(VoidResponder<'a, S, I>),
    #[doc(hidden)]
    Poison,
}

impl<'a, S, I> BorrowedResponder<'a, S, I> {
    /// Wrap an HTTP client and interaction reference in a new responder
    #[inline]
    #[must_use]
    pub fn new(http: &'a Http, int: &'a I) -> Self {
        Self::Init(InitResponder(ResponderCore {
            http,
            int,
            schema: PhantomData,
        }))
    }
}

impl<'a, S: Schema, I: private::Interaction> BorrowedResponder<'a, S, I> {
    /// Create a response message if this responder is in its initial state, or
    /// create a followup message if a response has already been created
    ///
    /// # Errors
    /// This method returns an error if the message contains errors or an API
    /// error is received.
    ///
    /// # Panics
    /// This method panics if the responder has entered a poisoned state.  This
    /// should not happen unless a thread panicked in the middle of a state
    /// update.
    pub async fn create_or_followup(
        &mut self,
        msg: Message<S::Component, id::Error>,
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

/// A responder that mutates a [`BorrowedResponder`] when used, to synchronize
/// type-states outside of a handler function
#[derive(Debug)]
pub struct BorrowingResponder<'a, 'b, S, I>(&'a mut BorrowedResponder<'b, S, I>);

impl<'a, 'b, S, I> BorrowingResponder<'a, 'b, S, I> {
    /// Borrow an existing [`BorrowedResponder`]
    ///
    /// # Panics
    /// This function panics if the borrowed responder is not in its
    /// [`Init`](BorrowedResponder::Init) state.
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
    /// Create a channel message response
    ///
    /// # Errors
    /// This method returns an error if the message contains errors or an API
    /// error is received.
    #[inline]
    pub async fn create_message(
        self,
        msg: Message<S::Component, id::Error>,
    ) -> Result<CreatedResponder<'b, S, I>, ResponseError> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.create_message(msg)).await }
    }

    /// Create a deferred channel message response
    ///
    /// # Errors
    /// This method returns an error if an API error is received.
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
    /// Create a message update response
    ///
    /// # Errors
    /// This method returns an error if the message contains errors or an API
    /// error is received.
    #[inline]
    pub async fn update_message(
        self,
        msg: Message<S::Component, id::Error>,
    ) -> Result<CreatedResponder<'b, S, I>, ResponseError> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.update_message(msg)).await }
    }

    /// Create a deferred message update response
    ///
    /// # Errors
    /// This method returns an error if an API error is received.
    #[inline]
    pub async fn defer_update(self) -> Result<CreatedResponder<'b, S, I>, serenity::Error> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(super::InitResponder::defer_update).await }
    }
}

impl<'a, 'b, S: Schema, I: private::TryCreateUpdate> BorrowingResponder<'a, 'b, S, I> {}

impl<'a, 'b, S: Schema, I: private::CreateModal> BorrowingResponder<'a, 'b, S, I> {
    /// Create a modal dialog response
    ///
    /// # Errors
    /// This method returns an error if the modal contains errors or an API
    /// error is received.
    #[inline]
    pub async fn modal(
        self,
        f: impl FnOnce(ModalSourceHandle) -> Modal<S, id::Error>,
    ) -> Result<VoidResponder<'b, S, I>, ResponseError> {
        // SAFETY: this is a create response endpoint
        unsafe { self.take(|i| i.modal(f)).await }
    }
}

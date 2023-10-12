use std::time::Duration;

use paracord::mention::MentionableTimestamp;
use serenity::model::prelude::{UserId, GuildId, InteractionId};

use crate::prelude::*;

use super::{Outcome, LastAccusation};

// TODO: we might be able to replace this with a concrete type like memory::DiscordTime later
pub trait Timestamp: Sized + Send + Sync {
    fn mentionable(&self) -> Option<MentionableTimestamp>;
    fn checked_add(&self, duration: Duration) -> Option<Self>;
    fn of_interaction(interaction: InteractionId) -> Self;
}

#[async_trait]
pub trait AccusationTransaction: Send + Sync {
    type Timestamp: Timestamp;
    type ResolveFuture: Future<Output=Outcome> + Send + 'static;

    fn last_accusation(&self) -> Option<LastAccusation>;
    fn accusation_time(&self) -> Self::Timestamp;
    async fn commit(self) -> Self::ResolveFuture; // TODO: when we have a database this will probably need to become a result as well... sucks though
}

pub enum RecordOutcomeError {
    AlreadyResolved(Outcome),
    NotAccused,
    Other(anyhow::Error)
}

#[async_trait]
pub trait SleeperStorage {
    type Timestamp: Timestamp;
    type Transaction<'a>: AccusationTransaction<Timestamp = Self::Timestamp> where Self: 'a;
    
    async fn last_accusation(&self, guild: GuildId, user: UserId) -> Result<Option<LastAccusation>>;
    async fn begin_accuse<'a>(&'a self, guild: GuildId, user: UserId, time: Self::Timestamp) -> Result<Self::Transaction<'a>>;
    async fn record_outcome(&self, guild: GuildId, user: UserId, time: Self::Timestamp, outcome: Outcome) -> Result<(), RecordOutcomeError>;
}

mod memory {
    use super::*;

    use std::{ops::Deref, ops::DerefMut, pin::Pin};

    use futures_util::TryFuture;
    use tokio::sync::{oneshot, RwLock, Mutex, RwLockWriteGuard, OwnedMutexGuard};

    #[derive(Clone)]
    pub struct DiscordTime(serenity::model::Timestamp);

    impl super::Timestamp for DiscordTime {
        fn mentionable(&self) -> Option<MentionableTimestamp> {
            MentionableTimestamp::from_seconds_since_unix_epoch(self.0.unix_timestamp())
        }

        fn checked_add(&self, duration: Duration) -> Option<Self> {
            Some(DiscordTime(self.0.checked_add_signed(chrono::Duration::from_std(duration).ok()?)?.into())) // this is a bit ugly
        }

        fn of_interaction(interaction: InteractionId) -> Self {
            DiscordTime(interaction.created_at())
        }
    }

    enum Accusation {
        Unresolved { time_accused: DiscordTime, resolve: oneshot::Sender<Outcome> },
        Resolved { time_accused: DiscordTime, time_resolved: DiscordTime, outcome: Outcome },
    }

    impl Accusation {
        // TODO: resolve naming here
        fn into_last_accusation(&self) -> LastAccusation {
            todo!()
            /*
            match self {
                Accusation::Unresolved { time_accused, .. } => LastAccusation { time_accused: *time_accused, outcome_and_time_resolved: None },
                Accusation::Resolved { time_accused, time_resolved, outcome } => LastAccusation { time_accused: *time_accused, outcome_and_time_resolved: Some((*outcome, *time_resolved)) }
            }
             */
        }
    }

    type AccusationTable = HashMap<(GuildId, UserId), Arc<Mutex<Accusation>>>;

    pub struct MemorySleeperStorage {
        accusations: RwLock<AccusationTable>
    }

    impl MemorySleeperStorage {
        async fn get_accusation_mutex(&self, guild: GuildId, user: UserId) -> Option<Arc<Mutex<Accusation>>> {
            self.accusations.read().await.get(&(guild, user)).cloned()
        }

        async fn _last_accusation(&self, guild: GuildId, user: UserId) -> Option<LastAccusation> {
            Some(self.get_accusation_mutex(guild, user).await?.lock().await.into_last_accusation())
        }
    }

    pub struct Transaction<'a> {
        kind: TransactionKind<'a>,
        guild: GuildId, 
        user: UserId,
        time_accused: DiscordTime, // TODO: rename this
    }

    enum TransactionKind<'a> {
        Replacing(OwnedMutexGuard<Accusation>),
        Creating(RwLockWriteGuard<'a, AccusationTable>)
    }

    pub enum OkOrNever<F> { Polling(F), Never }

    impl<F> OkOrNever<F> {
        pub fn new(f: F) -> Self { OkOrNever::Polling(f) }
    }

    impl<F: TryFuture> Future for OkOrNever<F> {
        type Output = F::Ok;

        fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
            use std::task::Poll;
            let this = unsafe { self.get_unchecked_mut() };
            match this {
                OkOrNever::Polling(fut) => match unsafe { Pin::new_unchecked(fut) }.try_poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Ok(r)) => Poll::Ready(r),
                    Poll::Ready(Err(_)) => { *this = OkOrNever::Never; Poll::Pending }
                }
                OkOrNever::Never => Poll::Pending
            }
        }
    }

    #[async_trait]
    impl<'a> AccusationTransaction for Transaction<'a> {
        type Timestamp = DiscordTime;
        type ResolveFuture = OkOrNever<oneshot::Receiver<Outcome>>;

        fn accusation_time(&self) -> DiscordTime { self.time_accused.clone() }

        fn last_accusation(&self) -> Option<LastAccusation> {
            match &self.kind {
                TransactionKind::Replacing(guard) => Some(guard.into_last_accusation()),
                TransactionKind::Creating(_) => None,
            }
        }

        async fn commit(self) -> Self::ResolveFuture {
            let Transaction { kind, guild, user, time_accused } = self;
            let (sender, receiver) = oneshot::channel();
            let accusation = Accusation::Unresolved { time_accused, resolve: sender };
            match kind {
                TransactionKind::Replacing(mut guard) => { *guard = accusation },
                TransactionKind::Creating(mut guard) => { guard.insert((guild, user), Arc::new(Mutex::new(accusation))); }
            }
            OkOrNever::new(receiver)
        }
    }

    #[async_trait]
    impl SleeperStorage for MemorySleeperStorage {
        type Timestamp = DiscordTime;
        type Transaction<'a> = Transaction<'a>;

        async fn last_accusation(&self, guild: GuildId, user: UserId) -> Result<Option<LastAccusation>> { Ok(self._last_accusation(guild, user).await) }

        async fn begin_accuse<'a>(&'a self, guild: GuildId, user: UserId, time_accused: DiscordTime) -> Result<Self::Transaction<'a>> {
            let mutex = match self.get_accusation_mutex(guild, user).await {
                Some(mutex) => mutex,
                None => {
                    let guard = self.accusations.write().await;
                    match guard.deref().get(&(guild, user)).cloned() {
                        Some(mutex) => mutex,
                        None => return Ok(Transaction { kind: TransactionKind::Creating(guard), guild, user, time_accused })
                    }
                }
            };

            Ok(Transaction { kind: TransactionKind::Replacing(mutex.lock_owned().await), guild, user, time_accused})
        }

        async fn record_outcome(&self, guild: GuildId, user: UserId, time_resolved: DiscordTime, outcome: Outcome) -> Result<(), RecordOutcomeError> {
            let Some(mutex) = self.get_accusation_mutex(guild, user).await else { return Err(RecordOutcomeError::NotAccused) };
            let mut guard = mutex.lock().await;
            let accusation = guard.deref_mut();
            match accusation {
                Accusation::Resolved { outcome, .. } => Err(RecordOutcomeError::AlreadyResolved(*outcome)),
                Accusation::Unresolved { time_accused, .. } => {
                    let time_accused = time_accused.clone();
                    let Accusation::Unresolved { resolve, .. } = std::mem::replace(accusation, Accusation::Resolved { time_accused, time_resolved, outcome }) else { unreachable!() };
                    let _ = resolve.send(outcome); // normal for this to err if, for example, we are being called with Outcome::TheSleeper because the timer won the race against the other half of resolve
                    Ok(())
                }
            }
        }
    }
}
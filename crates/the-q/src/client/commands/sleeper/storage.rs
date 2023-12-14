use std::time::Duration;

use paracord::mention::MentionableTimestamp;
use serenity::model::prelude::*;

use super::{AccusationInfo, Outcome};
use crate::prelude::*;

// TODO: we might be able to replace this with a concrete type like memory::DiscordTime later
pub trait Timestamp: Sized + Send + Sync {
    fn mentionable(&self) -> Option<MentionableTimestamp>;
    fn checked_add(&self, duration: Duration) -> Option<Self>;
    fn saturating_duration_since(&self, other: &Self) -> Duration;
    fn of_interaction(interaction: InteractionId) -> Self;
}

#[async_trait]
pub trait AccusationTransaction: Send + Sync {
    type Timestamp: Timestamp;
    type ResolveFuture: Future<Output = Outcome> + Send + 'static;

    fn last_accusation(&self) -> Option<AccusationInfo<Self::Timestamp>>;
    fn accusation_time(&self) -> Self::Timestamp;
    async fn commit(self) -> Self::ResolveFuture; // TODO: when we have a database this will probably need to become a result as well... sucks though
}

pub enum RecordOutcomeError {
    AlreadyResolved(Outcome),
    NotAccused,
    Other(anyhow::Error),
}

#[async_trait]
pub trait SleeperStorage {
    type Timestamp: Timestamp;
    type Transaction<'a>: AccusationTransaction<Timestamp = Self::Timestamp>
    where Self: 'a;

    async fn last_accusation(
        &self,
        guild: GuildId,
        user: UserId,
    ) -> Result<Option<AccusationInfo<Self::Timestamp>>>;

    async fn begin_accuse<'a>(
        &'a self,
        guild: GuildId,
        user: UserId,
        time: Self::Timestamp,
    ) -> Result<Self::Transaction<'a>>;

    async fn record_outcome(
        &self,
        guild: GuildId,
        user: UserId,
        time: Self::Timestamp,
        outcome: Outcome,
    ) -> Result<(), RecordOutcomeError>;
}

mod memory {
    use std::{ops::Deref, pin::Pin, time::Duration};

    use futures_util::TryFuture;
    use paracord::mention::MentionableTimestamp;
    use serenity::model::prelude::*;
    use tokio::sync::{oneshot, Mutex, OwnedMutexGuard, RwLock, RwLockWriteGuard};

    use super::{
        AccusationInfo, AccusationTransaction, Outcome, RecordOutcomeError, SleeperStorage,
    };
    use crate::prelude::*;

    // TODO: converting between the various types here is kind of ugly. We could either:
    // - switch to using `chrono::Duration` instead of `std::time::Duration` ourselves
    // - drop chrono and serenity's Timestamp entirely and define our own type for a "discord snowflake time"
    #[derive(Clone)]
    pub struct DiscordTime(serenity::model::Timestamp);

    impl super::Timestamp for DiscordTime {
        fn mentionable(&self) -> Option<MentionableTimestamp> {
            MentionableTimestamp::from_seconds_since_unix_epoch(self.0.unix_timestamp())
        }

        fn checked_add(&self, duration: Duration) -> Option<Self> {
            Some(DiscordTime(
                self.0
                    .checked_add_signed(chrono::Duration::from_std(duration).ok()?)?
                    .into(),
            ))
        }

        fn saturating_duration_since(&self, other: &Self) -> Duration {
            self.0
                .signed_duration_since(*other.0)
                .to_std()
                .unwrap_or(Duration::ZERO)
        }

        fn of_interaction(interaction: InteractionId) -> Self {
            DiscordTime(interaction.created_at())
        }
    }

    enum Accusation {
        Unresolved {
            time_accused: DiscordTime,
            resolve: oneshot::Sender<Outcome>,
        },
        Resolved {
            time_accused: DiscordTime,
            time_resolved: DiscordTime,
            outcome: Outcome,
        },
    }

    impl From<&Accusation> for AccusationInfo<DiscordTime> {
        fn from(value: &Accusation) -> Self {
            match value {
                Accusation::Unresolved { time_accused, .. } => AccusationInfo {
                    time_accused: time_accused.clone(),
                    outcome_and_time_resolved: None,
                },
                Accusation::Resolved {
                    time_accused,
                    time_resolved,
                    outcome,
                } => AccusationInfo {
                    time_accused: time_accused.clone(),
                    outcome_and_time_resolved: Some((*outcome, time_resolved.clone())),
                },
            }
        }
    }

    type AccusationTable = HashMap<(GuildId, UserId), Arc<Mutex<Accusation>>>;

    pub struct MemorySleeperStorage {
        accusations: RwLock<AccusationTable>,
    }

    impl MemorySleeperStorage {
        async fn get_accusation_mutex(
            &self,
            guild: GuildId,
            user: UserId,
        ) -> Option<Arc<Mutex<Accusation>>> {
            self.accusations.read().await.get(&(guild, user)).cloned()
        }

        async fn _last_accusation(
            &self,
            guild: GuildId,
            user: UserId,
        ) -> Option<AccusationInfo<DiscordTime>> {
            Some(
                self.get_accusation_mutex(guild, user)
                    .await?
                    .lock()
                    .await
                    .deref()
                    .into(),
            )
        }
    }

    pub struct Transaction<'a> {
        kind: TransactionKind<'a>,
        guild: GuildId,
        user: UserId,
        time_accused: DiscordTime, // TODO: is this the best name for this field?
    }

    enum TransactionKind<'a> {
        Replacing(OwnedMutexGuard<Accusation>),
        Creating(RwLockWriteGuard<'a, AccusationTable>),
    }

    pub enum OkOrNever<F> {
        Polling(F),
        Never,
    }

    impl<F> OkOrNever<F> {
        pub fn new(f: F) -> Self { OkOrNever::Polling(f) }
    }

    impl<F: TryFuture> Future for OkOrNever<F> {
        type Output = F::Ok;

        fn poll(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            use std::task::Poll;
            let this = unsafe { self.get_unchecked_mut() };
            match this {
                OkOrNever::Polling(fut) => match unsafe { Pin::new_unchecked(fut) }.try_poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Ok(r)) => Poll::Ready(r),
                    Poll::Ready(Err(_)) => {
                        *this = OkOrNever::Never;
                        Poll::Pending
                    },
                },
                OkOrNever::Never => Poll::Pending,
            }
        }
    }

    #[async_trait]
    impl<'a> AccusationTransaction for Transaction<'a> {
        type ResolveFuture = OkOrNever<oneshot::Receiver<Outcome>>;
        type Timestamp = DiscordTime;

        fn accusation_time(&self) -> DiscordTime { self.time_accused.clone() }

        fn last_accusation(&self) -> Option<AccusationInfo<DiscordTime>> {
            match &self.kind {
                TransactionKind::Replacing(guard) => Some(guard.deref().into()),
                TransactionKind::Creating(_) => None,
            }
        }

        async fn commit(self) -> Self::ResolveFuture {
            let Transaction {
                kind,
                guild,
                user,
                time_accused,
            } = self;

            let (sender, receiver) = oneshot::channel();

            let accusation = Accusation::Unresolved {
                time_accused,
                resolve: sender,
            };

            match kind {
                TransactionKind::Replacing(mut guard) => *guard = accusation,
                TransactionKind::Creating(mut guard) => {
                    guard.insert((guild, user), Arc::new(Mutex::new(accusation)));
                },
            }

            OkOrNever::new(receiver)
        }
    }

    #[async_trait]
    impl SleeperStorage for MemorySleeperStorage {
        type Timestamp = DiscordTime;
        type Transaction<'a> = Transaction<'a>;

        async fn last_accusation(
            &self,
            guild: GuildId,
            user: UserId,
        ) -> Result<Option<AccusationInfo<DiscordTime>>> {
            Ok(self._last_accusation(guild, user).await)
        }

        async fn begin_accuse<'a>(
            &'a self,
            guild: GuildId,
            user: UserId,
            time_accused: DiscordTime,
        ) -> Result<Self::Transaction<'a>> {
            let mutex = if let Some(mutex) = self.get_accusation_mutex(guild, user).await {
                mutex
            } else {
                let guard = self.accusations.write().await;
                match guard.deref().get(&(guild, user)).cloned() {
                    Some(mutex) => mutex,
                    None => {
                        return Ok(Transaction {
                            kind: TransactionKind::Creating(guard),
                            guild,
                            user,
                            time_accused,
                        });
                    },
                }
            };

            Ok(Transaction {
                kind: TransactionKind::Replacing(mutex.lock_owned().await),
                guild,
                user,
                time_accused,
            })
        }

        async fn record_outcome(
            &self,
            guild: GuildId,
            user: UserId,
            time_resolved: DiscordTime,
            outcome: Outcome,
        ) -> Result<(), RecordOutcomeError> {
            let Some(mutex) = self.get_accusation_mutex(guild, user).await else {
                return Err(RecordOutcomeError::NotAccused);
            };

            let mut guard = mutex.lock().await;
            let accusation = &mut *guard;
            match accusation {
                Accusation::Resolved { outcome, .. } => {
                    Err(RecordOutcomeError::AlreadyResolved(*outcome))
                },
                Accusation::Unresolved { time_accused, .. } => {
                    let time_accused = time_accused.clone();
                    let Accusation::Unresolved { resolve, .. } =
                        std::mem::replace(accusation, Accusation::Resolved {
                            time_accused,
                            time_resolved,
                            outcome,
                        })
                    else {
                        unreachable!()
                    };
                    let _ = resolve.send(outcome); // normal for this to err if, for example, we are being called with Outcome::TheSleeper because the timer won the race against the other half of resolve
                    Ok(())
                },
            }
        }
    }
}

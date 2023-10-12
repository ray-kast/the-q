use std::{fmt::Debug, time::Duration};

use futures_lite::FutureExt as _;
use futures_util::FutureExt as _;
use paracord::mention::TimestampStyle;
use serenity::{
    model::{prelude::ChannelId, voice::VoiceState},
    prelude::Mentionable,
};

use super::prelude::*;

mod storage;
use storage::{AccusationTransaction, RecordOutcomeError, SleeperStorage, Timestamp};

pub const GRACE_PERIOD: Duration = Duration::from_secs(25 * 60); // TODO: better name
pub const COOLDOWN_PERIOD: Duration = Duration::from_secs(45 * 60);

#[derive(Copy, Clone)]
pub enum Outcome {
    TheSleeper,
    NotTheSleeper,
}

pub struct AccusationInfo<T> {
    pub time_accused: T,
    pub outcome_and_time_resolved: Option<(Outcome, T)>,
}

enum SleeperAction {
    Disconnect,
    MoveToChannel(ChannelId),
}

#[derive(Debug)]
pub struct SleeperCommand<S> {
    name: String,
    storage: S,
}

impl<S: Default> From<&CommandOpts> for SleeperCommand<S> {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Fell asleep in call", opts.context_menu_base),
            storage: Default::default(), // TODO
        }
    }
}

impl<S: SleeperStorage> SleeperCommand<S> {
    const ACCUSE_NAME: &str = "accuse";
    const AWAKE_NAME: &str = "awake";

    fn sleeper_action(&self, ctx: &Context, guild: GuildId) -> Result<SleeperAction> {
        // TODO: make this configurable
        Ok(
            match ctx
                .cache
                .guild_field(guild, |guild| guild.afk_channel_id)
                .context("Could not determiner sleeper action")?
            {
                Some(afk_channel_id) => SleeperAction::MoveToChannel(afk_channel_id),
                None => SleeperAction::Disconnect,
            },
        )
    }

    #[allow(clippy::too_many_lines)]
    async fn accuse<'a>(
        &self,
        ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let user = visitor.user();
        let (target_user, _) = visitor.target().user()?;
        let (gid, _) = visitor.guild()?.required()?;

        /* restrict the lifetime of voice_states */
        {
            let voice_states = ctx
                .cache
                .guild_field(gid, |guild| guild.voice_states.clone())
                .context("Missing guild")?;

            let Some(channel_id) = voice_states.get(&user.id).and_then(|c| c.channel_id) else {
                return Err(responder
                    .create_message(
                        Message::plain("Please connect to a voice channel first.").ephemeral(true),
                    )
                    .await
                    .context("Error sending error message")?
                    .into_err("Error getting user voice channel"));
            };

            if voice_states.get(&target_user.id).and_then(|c| c.channel_id) != Some(channel_id) {
                return Err(responder
                    .create_message(
                        Message::rich(|builder| {
                            builder
                                .user(target_user)
                                .push(" is not in your voice channel!")
                        })
                        .ephemeral(true),
                    )
                    .await
                    .context("Error sending error message")?
                    .into_err("Error getting target user voice channel"));
            }
        }

        if target_user.bot {
            return Err(responder
                .create_message(Message::plain("Bots never sleep, silly!").ephemeral(true))
                .await
                .context("Error sending bot user error")?
                .into_err("Target user was a bot"));
        }

        let transaction = self
            .storage
            .begin_accuse(gid, target_user.id, Timestamp::of_interaction(visitor.id()))
            .await
            .context("Error accusing user")?;

        let deadline = transaction
            .accusation_time()
            .checked_add(GRACE_PERIOD)
            .expect("deadline is not in the distant past or future");

        // TODO: fancier messages for errors, show remaining time
        let responder = match transaction.last_accusation() {
            Some(AccusationInfo {
                outcome_and_time_resolved: None,
                time_accused,
            }) if transaction
                .accusation_time()
                .saturating_duration_since(&time_accused)
                <= GRACE_PERIOD =>
            {
                return Err(responder
                    .create_message(
                        Message::rich(|builder| {
                            builder.user(target_user).push(" has already been accused!")
                        })
                        .ephemeral(true),
                    )
                    .await
                    .context("Error sending already accused error")?
                    .into_err("User has already been accused"));
            },
            Some(AccusationInfo {
                outcome_and_time_resolved: Some((_, time_resolved)),
                ..
            }) if transaction
                .accusation_time()
                .saturating_duration_since(&time_resolved)
                <= COOLDOWN_PERIOD =>
            {
                return Err(responder
                    .create_message(
                        Message::rich(|builder| {
                            builder
                                .user(target_user)
                                .push(" has been accused too recently!")
                        })
                        .ephemeral(true),
                    )
                    .await
                    .context("Error sending accused-too-recently error")?
                    .into_err("Target user has been accused too recently"));
            },
            _ => {
                responder
                    .create_message(
                        Message::rich(
                            |builder| {
                                builder
                                    .push_line(format_args!(
                                        "{user} has accused {target_user} of being The Sleeper!",
                                        user = user.mention(),
                                        target_user = target_user.mention()
                                    ))
                                    .push(format_args!(
                                        "{target_user} will be The Sleeper {deadline} unless they \
                                         run ",
                                        target_user = target_user.mention(),
                                        deadline = deadline
                                            .mentionable()
                                            .expect("deadline is not in the distant past or future")
                                            .mention(TimestampStyle::RelativeTime) // TODO: handle this
                                    ))
                                    .push_mono_safe(format!("/{} {}", self.name, Self::AWAKE_NAME))
                                    .push_line(" first!")
                            }, // TODO: use a command mention here
                        )
                        .ping_users(vec![target_user.id]),
                    )
                    .await
                    .context("Error sending accusation notification")?
            },
        };

        // TODO: past this point we may want to edit the message if an error occurs
        let resolve = transaction.commit().await;

        let expire = async {
            tokio::time::sleep(GRACE_PERIOD).await;

            match self
                .storage
                .record_outcome(gid, user.id, deadline, Outcome::TheSleeper)
                .await
            {
                Ok(()) => Ok(Outcome::TheSleeper),
                Err(RecordOutcomeError::AlreadyResolved(outcome)) => Ok(outcome), // TODO: any reason to log this? it can only happen when there's a race condition
                Err(RecordOutcomeError::NotAccused) => {
                    Err(anyhow!("Accusation was missing at time of expiry"))
                },
                Err(RecordOutcomeError::Other(e)) => Err(e), // type inference fails when using ? to deal with this for some reason
            }
            .context("Error recording outcome")
        };

        match resolve.map(Ok).or(expire).await? {
            Outcome::TheSleeper => {
                let build_msg = || {
                    Message::rich(|builder| {
                        builder.push_line(format_args!(
                            "{target_user} is The Sleeper!",
                            target_user = target_user.mention()
                        ))
                    })
                    .ping_users(vec![target_user.id])
                };

                responder
                    .create_followup(build_msg())
                    .or_else(|e| async { Err(e) } /* TODO: try to send a fresh message in case the interaction has expired */)
                    .await.context("Error sending resolution announcement")?;

                let voice_state = ctx
                    .cache
                    .guild_field(gid, |guild| {
                        guild.voice_states.get(&target_user.id).cloned()
                    })
                    .context("Missing guild")?;
                if let Some(VoiceState {
                    channel_id: Some(_),
                    ..
                }) = voice_state
                {
                    match self.sleeper_action(ctx, gid)? {
                        SleeperAction::Disconnect => gid
                            .disconnect_member(&ctx.http, target_user)
                            .await
                            .context("Error disconnecting user"),
                        SleeperAction::MoveToChannel(afk_channel_id) => gid
                            .move_member(&ctx.http, target_user, afk_channel_id)
                            .await
                            .context("Error moving user to AFK channel"),
                    }
                    .map_err(|e| error!(?e))
                    .ok(); // TODO: use inspect_err when available
                }
            },
            Outcome::NotTheSleeper => {
                let build_msg = || {
                    Message::rich(|builder| {
                        builder.push_line(format_args!(
                            "{target_user} was not The Sleeper! Sorry, {user}!",
                            target_user = target_user.mention(),
                            user = user.mention()
                        ))
                    })
                    .ping_users(vec![user.id])
                };

                responder
                    .create_followup(build_msg())
                    .or_else(|e| async { Err(e) } /* TODO: try to send a fresh message in case the interaction has expired */)
                    .await.context("Error sending resolution announcement")?;
            },
        };

        Ok(responder.into())
    }

    async fn awake<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let user = visitor.user();
        let (gid, _) = visitor.guild()?.required()?;

        let time_awake = S::Timestamp::of_interaction(visitor.id());
        let result = match self.storage.last_accusation(gid, user.id).await? {
            Some(AccusationInfo {
                outcome_and_time_resolved: None,
                time_accused,
            }) if time_awake.saturating_duration_since(&time_accused) <= GRACE_PERIOD => {
                self.storage
                    .record_outcome(
                        gid,
                        user.id,
                        Timestamp::of_interaction(visitor.id()),
                        Outcome::NotTheSleeper,
                    )
                    .await
            },
            Some(AccusationInfo {
                outcome_and_time_resolved: Some((outcome, time_resolved)),
                ..
            }) if time_awake.saturating_duration_since(&time_resolved) <= COOLDOWN_PERIOD => {
                Err(RecordOutcomeError::AlreadyResolved(outcome))
            },
            _ => Err(RecordOutcomeError::NotAccused),
        };

        // TODO: make the message in the success case a little fancier?
        let responder = match result {
            Ok(()) => responder
                .create_message(
                    Message::plain(
                        "You have successfully refuted the accusation of being The Sleeper.",
                    )
                    .ephemeral(true),
                )
                .await
                .context("Error sending refutation acknowledgement")?,
            Err(RecordOutcomeError::AlreadyResolved(Outcome::TheSleeper)) => {
                return Err(responder
                    .create_message(
                        Message::plain("Sorry, you were too late to clear your name this time.")
                            .ephemeral(true),
                    )
                    .await
                    .context("Error sending too-late error")?
                    .into_err("User was too late to defend themself"));
            },
            Err(RecordOutcomeError::AlreadyResolved(Outcome::NotTheSleeper)) => {
                return Err(responder
                    .create_message(
                        Message::plain("You have already cleared your name this time.")
                            .ephemeral(true),
                    )
                    .await
                    .context("Error sending already-resolved error")?
                    .into_err("User had already indicated they were awake"));
            },
            Err(RecordOutcomeError::NotAccused) => {
                return Err(responder
                    .create_message(
                        Message::plain("You are not currently accused of being The Sleeper.")
                            .ephemeral(true),
                    )
                    .await
                    .context("Error sending not-accused error")?
                    .into_err("User had not been accused"));
            },
            Err(RecordOutcomeError::Other(e)) => return Err(e.into()),
        };

        Ok(responder.into())
    }
}

#[async_trait]
impl<S: SleeperStorage + Debug + Sync + Send + 'static> CommandHandler<Schema>
    for SleeperCommand<S>
{
    fn register_global(&self) -> CommandInfo { CommandInfo::user(&self.name) }

    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        match *visitor.visit_subcmd()? {
            [Self::ACCUSE_NAME] => self.accuse(ctx, visitor, responder).await,
            [Self::AWAKE_NAME] => self.awake(ctx, visitor, responder).await,
            [..] => unreachable!(), // TODO: visitor should handle this
        }
    }
}

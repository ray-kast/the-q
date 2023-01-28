use std::path::PathBuf;

use super::prelude::*;

#[derive(Debug)]
pub struct VcCommand;

#[async_trait]
impl Handler for VcCommand {
    fn register_global(&self, opts: &handler::Opts) -> CommandInfo {
        CommandInfo::build_slash(&opts.command_base, ";)", |a| {
            a.string("path", "Path to the file to play", true, ..)
                .autocomplete(true, ["path"])
        })
        .unwrap()
    }

    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let (gid, _memb) = visitor.guild().required()?;
        let user = visitor.user();
        let path = visitor.visit_string("path")?.required()?;

        let guild = gid.to_guild_cached(&ctx.cache).context("Missing guild")?;

        let Some(voice_chan) = guild.voice_states.get(&user.id).and_then(|s| s.channel_id)
        else {
            return Err(responder
                .create_message(
                    Message::plain("Please connect to a voice channel first.").ephemeral(true),
                )
                .await
                .context("Error sending voice channel error")?
                .into_err("Error getting user voice state"));
        };

        let responder = responder
            .defer_message(MessageOpts::default().ephemeral(true))
            .await
            .context("Error sending deferred message")?;

        let sb = songbird::get(ctx)
            .await
            .context("Missing songbird context")?;

        let (call_lock, res) = sb.join(gid, voice_chan).await;

        if let Err(err) = res {
            warn!(?err, "Unable to join voice channel");
            responder
                .edit(MessageBody::plain("Couldn't join that channel, sorry."))
                .await
                .context("Error sending channel join error")?;

            return Err(responder.into_err("Error joining call (missing permissions?)"));
        }

        let mut call = call_lock.lock().await;

        let path = {
            const PREFIX: &str = "etc/samples";
            let mut p = PathBuf::from(PREFIX);

            p.push(path);

            if !p.starts_with(PREFIX) {
                responder
                    .edit(MessageBody::plain("Oops!  That's not a valid path."))
                    .await
                    .context("Error sending bad path error")?;

                return Err(responder.into_err("Path error - escaped sample directory"));
            }

            p
        };

        if tokio::fs::metadata(&path).await.is_err() {
            responder
                .edit(MessageBody::plain("That isn't a valid file."))
                .await
                .context("Error sending bad stat error")?;

            return Err(responder.into_err("Stat error for file"));
        }

        let source = songbird::ffmpeg(path)
            .await
            .context("Error opening sample")?;

        call.play_source(source)
            .add_event(
                songbird::Event::Track(songbird::TrackEvent::End),
                SongbirdHandler(Arc::clone(&call_lock)),
            )
            .context("Error hooking track stop")?;

        responder
            .edit(MessageBody::plain(";)").build_row(|c| {
                c.link_button(
                    Url::parse("https://youtu.be/dQw4w9WgXcQ").unwrap(),
                    "See More",
                    false,
                )
            }))
            .await
            .context("Error updating deferred response")?;

        Ok(responder.into())
    }
}

struct SongbirdHandler(Arc<tokio::sync::Mutex<songbird::Call>>);

#[async_trait]
impl songbird::EventHandler for SongbirdHandler {
    async fn act(&self, ctx: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        match *ctx {
            songbird::EventContext::Track(t) => {
                if t.iter().all(|(s, _)| s.playing.is_done()) {
                    self.0
                        .lock()
                        .await
                        .leave()
                        .await
                        .map_err(|err| error!(%err, "Error leaving call"))
                        .ok();
                }

                None
            },
            _ => None,
        }
    }
}

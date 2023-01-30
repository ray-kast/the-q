use std::{collections::BinaryHeap, path::PathBuf};

use ordered_float::OrderedFloat;
use tokio::sync::{mpsc, oneshot, RwLock};

use super::prelude::*;

// TODO: make this configurable
const SAMPLE_DIR: &str = "etc/samples";

#[derive(Debug)]
struct FileMap {
    files: RwLock<HashMap<String, PathBuf>>,
    task_handle: oneshot::Sender<Infallible>,
}

#[derive(Debug, Default)]
pub struct VcCommand {
    files: tokio::sync::Mutex<std::sync::Weak<FileMap>>,
    notify_handle: RwLock<Option<oneshot::Sender<()>>>,
}

impl VcCommand {
    async fn files(&self) -> Result<Arc<FileMap>> {
        let mut guard = self.files.lock().await;
        if let Some(files) = guard.upgrade() {
            return Ok(files);
        }

        let (task_handle, handle_rx) = oneshot::channel();

        let files = Arc::new(FileMap {
            files: RwLock::default(),
            task_handle,
        });

        let (ready_tx, ready_rx) = oneshot::channel();
        let map = Arc::clone(&files);
        tokio::task::spawn(
            async move {
                let mut ready_tx = Some(ready_tx);
                let (watch_tx, mut watch_rx) = mpsc::channel(8);

                watch_tx
                    .try_send(Ok(notify::Event::new(notify::EventKind::Any)))
                    .unwrap_or_else(|_| unreachable!());

                let watcher = tokio::task::spawn_blocking(move || {
                    use notify::Watcher;

                    let mut w = notify::recommended_watcher(move |r| {
                        // TODO: how to gracefully handle send error?
                        watch_tx.blocking_send(r).unwrap();
                    })
                    .context("Error creating filesystem watcher")?;

                    w.watch(SAMPLE_DIR.as_ref(), notify::RecursiveMode::Recursive)?;

                    Result::<_>::Ok(w)
                })
                .await
                .unwrap()?;

                let recv = async move {
                    while let Some(evt) = watch_rx.recv().await {
                        let evt = evt?;

                        info!(?evt, "Scanning sample table...");

                        let files = walkdir::WalkDir::new(SAMPLE_DIR)
                            .same_file_system(true)
                            .into_iter()
                            .filter_map(|f| {
                                f.map(|f| {
                                    (f.file_name()
                                        .to_str()
                                        .map_or(false, |s| !s.starts_with('.'))
                                        && f.metadata().map_or(false, |m| m.is_file()))
                                    .then(|| {
                                        let f = f.into_path();
                                        let s = f.display().to_string();
                                        let s = s.strip_prefix(SAMPLE_DIR).unwrap();
                                        let s = s.strip_prefix('/').unwrap_or(s);
                                        (s.into(), f)
                                    })
                                })
                                .transpose()
                            })
                            .collect::<Result<_, _>>()
                            .context("Error enumerating files")?;

                        info!(?files, "Sample table scan completed");

                        *map.files.write().await = files;
                        if let Some(tx) = ready_tx.take() {
                            tx.send(()).ok();
                        }
                    }

                    mem::drop((watcher, map, ready_tx));

                    Result::<_>::Ok(())
                };

                tokio::select! {
                    r = recv => r,
                    _ = handle_rx => Ok(()),
                }
            }
            .map_err(|err| error!(%err, "Sample watcher crashed"))
            .instrument(error_span!("watch_samples")),
        );

        ready_rx
            .await
            .context("Error getting initial sample table")?;
        *guard = Arc::downgrade(&files);

        Ok(files)
    }
}

#[async_trait]
impl Handler<Schema> for VcCommand {
    fn register_global(&self, opts: &handler::Opts) -> CommandInfo {
        CommandInfo::build_slash(&opts.command_base, ";)", |a| {
            a.string("path", "Path to the file to play", true, ..)
                .autocomplete(true, ["path"])
        })
        .unwrap()
    }

    async fn complete(&self, _: &Context, visitor: &mut CompletionVisitor<'_>) -> CompletionResult {
        // TODO: CompletionVisitor should probably have a better API
        // TODO: unicase?
        let path = visitor
            .visit_string("path")?
            .optional()
            .map(|s| s.to_lowercase());
        let path = path.as_deref().unwrap_or("");
        let files = self.files().await?;
        let files = files.files.read().await;

        #[allow(clippy::cast_precision_loss)]
        let mut heap: BinaryHeap<_> = {
            let all = once_cell::unsync::OnceCell::new();
            files
                .keys()
                .map(|s| {
                    (
                        OrderedFloat(strsim::normalized_damerau_levenshtein(
                            path,
                            &s.to_lowercase(),
                        )),
                        s,
                    )
                })
                .filter(|(s, _)| {
                    let matching = s.0 >= 0.07;
                    *all.get_or_init(|| !matching) || matching
                })
                .collect()
        };

        debug!(?heap, "File completion list accumulated");

        Ok(std::iter::from_fn(move || heap.pop())
            .map(|(_, s)| Completion {
                name: s.into(),
                value: s.into(),
            })
            .collect())
    }

    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        const PATH_ERR: &str = "That isn't a valid file.";

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

        let files = self.files().await.context("Error getting sample list")?;
        let files = files.files.read().await;
        let path = files.get(path);

        let Some(path) = path else {
            responder.edit(MessageBody::plain(PATH_ERR)).await.context("Error sending no path error")?;

            return Err(responder.into_err("File not in sample table"));
        };

        if tokio::fs::metadata(&path).await.is_err() {
            responder
                .edit(MessageBody::plain(PATH_ERR))
                .await
                .context("Error sending bad stat error")?;

            return Err(responder.into_err("Stat error for file"));
        }

        let source = songbird::ffmpeg(&path)
            .await
            .with_context(|| format!("Error opening sample {path:?}"))?;

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

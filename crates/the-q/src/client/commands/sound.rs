use std::{collections::BinaryHeap, path::PathBuf};

use ordered_float::OrderedFloat;
use paracord::interaction::visitor::Autocomplete;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

use super::prelude::*;

#[derive(Debug)]
struct FileMap {
    files: RwLock<HashMap<String, PathBuf>>,
    _task_handle: oneshot::Sender<Infallible>,
}

#[derive(Debug, Default)]
pub struct SoundCommand {
    files: Mutex<std::sync::Weak<FileMap>>,
    songbird_handle: Mutex<HashMap<GuildId, std::sync::Weak<()>>>,
    _notify_handle: RwLock<Option<oneshot::Sender<()>>>,
}

// #[derive(DeserializeCommand)]
// #[deserialize(cx = HandlerCx)]
pub enum SoundArgs<'a> {
    Play(PlayArgs<'a>),
    Board,
}

pub enum SoundCompletion<'a> {
    Play {
        path: Option<Autocomplete<'a, &'a str>>,
    },
}

impl<'a> DeserializeCommand<'a, HandlerCx> for SoundArgs<'a> {
    type Completion = SoundCompletion<'a>;

    fn register_global(cx: &HandlerCx) -> CommandInfo {
        CommandInfo::build_slash(cx.opts.command_name("sound"), ";)", |a| {
            a.build_subcmd("play", "Play a single file", |a| {
                a.string("path", "Path to the file to play", true, ..)
                    .autocomplete(true, ["path"])
            })
            .build_subcmd("board", "Create a soundboard message", |a| a)
        })
        .unwrap()
    }

    fn deserialize_completion(
        visitor: &mut CompletionVisitor<'a>,
    ) -> Result<Self::Completion, visitor::Error> {
        Ok(match *visitor.visit_subcmd()? {
            ["play"] => SoundCompletion::Play {
                path: visitor.visit_string_autocomplete("path")?.optional(),
            },
            _ => return Err(visitor::Error::Malformed("Unexpected subcommand")),
        })
    }

    fn deserialize(visitor: &mut CommandVisitor<'a>) -> Result<Self, visitor::Error> {
        Ok(match *visitor.visit_subcmd()? {
            ["play"] => Self::Play(PlayArgs {
                gid: visitor.guild()?.required()?.0,
                user: visitor.user(),
                path: visitor.visit_string("path")?.required()?,
            }),
            ["board"] => Self::Board,
            _ => return Err(visitor::Error::Malformed("Unexpected subcommand")),
        })
    }
}

pub struct PlayArgs<'a> {
    gid: GuildId,
    user: &'a User,
    path: &'a str,
}

// #[derive(DeserializeRpc)]
// #[deserialize(key = ComponentKey, cx = HandlerCx)]
pub struct SoundComponentArgs<'a> {
    gid: GuildId,
    user: &'a User,
}

impl<'a> DeserializeRpc<'a, ComponentKey, HandlerCx> for SoundComponentArgs<'a> {
    fn register_keys(_cx: &HandlerCx) -> &[ComponentKey] { &[ComponentKey::Soundboard] }

    fn deserialize(
        visitor: &mut visitor::BasicVisitor<'a, <ComponentKey as rpc::Key>::Interaction>,
    ) -> Result<Self, visitor::Error> {
        Ok(Self {
            gid: visitor.guild()?.required()?.0,
            user: visitor.user(),
        })
    }
}

impl SoundCommand {
    async fn files(&self, sample_dir: &str) -> Result<Arc<FileMap>> {
        let mut guard = self.files.lock().await;
        if let Some(files) = guard.upgrade() {
            return Ok(files);
        }

        let (task_handle, handle_rx) = oneshot::channel();

        let files = Arc::new(FileMap {
            files: RwLock::default(),
            _task_handle: task_handle,
        });

        let (ready_tx, ready_rx) = oneshot::channel();
        let map = Arc::clone(&files);
        let sample_dir = sample_dir.to_owned();
        tokio::task::spawn(
            async move {
                let mut ready_tx = Some(ready_tx);
                let (watch_tx, mut watch_rx) = mpsc::channel(8);

                watch_tx
                    .try_send(Ok(notify::Event::new(notify::EventKind::Any)))
                    .unwrap_or_else(|_| unreachable!());

                let watcher = tokio::task::spawn_blocking({
                    let sample_dir = sample_dir.clone();
                    move || {
                        use notify::Watcher;

                        let mut w = notify::recommended_watcher(move |r| {
                            // TODO: how to gracefully handle send error?
                            watch_tx.blocking_send(r).unwrap();
                        })
                        .context("Error creating filesystem watcher")?;

                        w.watch(sample_dir.as_ref(), notify::RecursiveMode::Recursive)?;

                        Result::<_>::Ok(w)
                    }
                })
                .await
                .unwrap()?;

                let sample_dir = &sample_dir;
                let recv = async move {
                    while let Some(evt) = watch_rx.recv().await {
                        let evt = evt?;

                        info!(?evt, "Scanning sample table...");

                        let files = walkdir::WalkDir::new(sample_dir)
                            .same_file_system(true)
                            .into_iter()
                            .filter_map(|f| {
                                f.map(|f| {
                                    (f.file_name().to_str().is_some_and(|s| !s.starts_with('.'))
                                        && f.metadata().is_ok_and(|m| m.is_file()))
                                    .then(|| {
                                        let f = f.into_path();
                                        let s = f.display().to_string();
                                        let s = s.strip_prefix(sample_dir).unwrap();
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
            .instrument(error_span!(parent: None, "watch_samples")),
        );

        ready_rx
            .await
            .context("Error getting initial sample table")?;
        *guard = Arc::downgrade(&files);

        Ok(files)
    }

    async fn play_impl<X, E: From<Error>, F: Future<Output = E>>(
        &self,
        serenity_cx: &Context,
        cx: &HandlerCx,
        args: PlayArgs<'_>,
        extra: X,
        fail: impl FnOnce(X, MessageBody, &'static str) -> F,
    ) -> Result<X, E> {
        const PATH_ERR: &str = "That isn't a valid file.";

        let PlayArgs { gid, user, path } = args;

        let voice_chan = {
            // gay baby jail to keep rustc from freaking out
            let guild = gid
                .to_guild_cached(&serenity_cx.cache)
                .context("Missing guild")?;
            guild.voice_states.get(&user.id).and_then(|s| s.channel_id)
        };

        let Some(voice_chan) = voice_chan else {
            return Err(fail(
                extra,
                MessageBody::plain("Please connect to a voice channel first."),
                "Error getting user voice state",
            )
            .await);
        };

        let sb = songbird::get(serenity_cx)
            .await
            .context("Missing songbird context")?;

        let files = self
            .files(&cx.opts.sample_dir)
            .await
            .context("Error getting sample list")?;
        let files = files.files.read().await;
        let path = files.get(path);

        let Some(path) = path else {
            return Err(fail(
                extra,
                MessageBody::plain(PATH_ERR),
                "File not in sample table",
            )
            .await);
        };

        if tokio::fs::metadata(&path).await.is_err() {
            return Err(fail(extra, MessageBody::plain(PATH_ERR), "Stat error for file").await);
        }

        let input = songbird::input::Input::from(songbird::input::File::new(path.clone()))
            .make_live_async()
            .await
            .with_context(|| format!("Error opening sample {}", path.display()))?;

        let call = match sb.join(gid, voice_chan).await {
            Ok(l) => l,
            Err(err) => {
                warn!(?err, "Unable to join voice channel");
                return Err(fail(
                    extra,
                    MessageBody::plain("Couldn't join that channel, sorry."),
                    "Error joining call (missing permissions?)",
                )
                .await);
            },
        };

        let mut call_lock = call.lock().await;
        let mut handles = self.songbird_handle.lock().await;

        if handles
            .get(&gid)
            .and_then(std::sync::Weak::upgrade)
            .is_some()
        {
            return Err(fail(
                extra,
                MessageBody::plain("Calm down, buddy"),
                "Sound already running",
            )
            .await);
        }

        let handle = Arc::new(());
        handles.insert(gid, Arc::downgrade(&handle));

        call_lock
            .play_input(input)
            .add_event(
                songbird::Event::Track(songbird::TrackEvent::End),
                SongbirdHandler(handle, Arc::clone(&call)),
            )
            .context("Error hooking track stop")?;

        Ok(extra)
    }

    #[inline]
    async fn play<'a>(
        &self,
        serenity_cx: &Context,
        cx: &HandlerCx,
        args: PlayArgs<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let responder = responder
            .defer_message(MessageOpts::default().ephemeral(true))
            .await
            .context("Error sending deferred message")?;

        let responder = self
            .play_impl(serenity_cx, cx, args, responder, |r, m, e| async move {
                match r.edit(m).await.context("Error sending error message") {
                    Ok(_) => r.into_err(e),
                    Err(e) => CommandError::from(e),
                }
            })
            .await?;

        responder
            .edit(MessageBody::plain(";)").buttons(|b| {
                b.link(
                    Url::parse("https://youtu.be/dQw4w9WgXcQ").unwrap(),
                    "See More",
                    false,
                )
            }))
            .await
            .context("Error updating deferred response")?;

        Ok(responder.into())
    }

    async fn board<'a>(
        &self,
        _serenity_cx: &Context,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let responder = responder
            .create_message(Message::plain(":3").buttons(|b| {
                b.button(
                    ComponentPayload::Soundboard(component::Soundboard {
                        file: "BUDDY.flac".into(),
                    }),
                    ButtonStyle::Primary,
                    "BUDDY",
                    false,
                )
            }))
            .await
            .context("Error sending soundboard message")?;

        Ok(responder.into())
    }
}

impl CommandHandler<Schema, HandlerCx> for SoundCommand {
    type Data<'a> = SoundArgs<'a>;

    async fn complete<'a>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: <Self::Data<'a> as DeserializeCommand<'a, HandlerCx>>::Completion,
    ) -> CompletionResult {
        match data {
            SoundCompletion::Play { path } => {
                let path = path.map_or("", Autocomplete::into_str);
                let files = self.files(&cx.opts.sample_dir).await?;
                let files = files.files.read().await;

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
            },
        }
    }

    async fn respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        match data {
            SoundArgs::Play(args) => self.play(serenity_cx, cx, args, responder).await,
            SoundArgs::Board => self.board(serenity_cx, responder).await,
        }
    }
}

impl RpcHandler<Schema, ComponentKey, HandlerCx> for SoundCommand {
    type Data<'a> = SoundComponentArgs<'a>;

    async fn respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        payload: <ComponentKey as rpc::Key>::Payload,
        data: Self::Data<'a>,
        responder: ComponentResponder<'a, 'r>,
    ) -> ComponentResult<'r> {
        let SoundComponentArgs { gid, user } = data;

        #[expect(
            clippy::match_wildcard_for_single_variants,
            reason = "This will have more variants in the future"
        )]
        match payload {
            ComponentPayload::Soundboard(s) => {
                let component::Soundboard { file } = s;

                let responder = responder
                    .defer_update()
                    .await
                    .context("Error sending deferred update")?;

                let responder = self
                    .play_impl(
                        serenity_cx,
                        cx,
                        PlayArgs {
                            gid,
                            user,
                            path: &file,
                        },
                        responder,
                        |r, m, e| async move {
                            match r.create_followup(Message::from(m).ephemeral(true)).await {
                                Ok(_) => r.into_err(e),
                                Err(e) => {
                                    Error::from(e).context("Error sending error message").into()
                                },
                            }
                        },
                    )
                    .await?;

                Ok(responder.into())
            },
            _ => unreachable!(), // TODO: set up an error for this
        }
    }
}

struct SongbirdHandler(
    #[expect(dead_code, reason = "This field is used as a drop canary")] Arc<()>,
    Arc<Mutex<songbird::Call>>,
);

#[async_trait::async_trait]
impl songbird::EventHandler for SongbirdHandler {
    async fn act(&self, ctx: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        match *ctx {
            songbird::EventContext::Track(t) => {
                if t.iter().all(|(s, _)| s.playing.is_done()) {
                    self.1
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

// TODO: coverage tests
#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod check_compat;
mod compat_pair;
mod git;
mod protoc;
mod schema;
mod union_find;

fn main() { entry::main(); }

mod entry {
    use std::{io::prelude::*, path::PathBuf};

    use anyhow::{Context, Result};
    use clap::Parser;
    use tracing_subscriber::{filter::LevelFilter, prelude::*};

    use crate::{
        check_compat::CompatLog,
        compat_pair::CompatPair,
        git, protoc,
        schema::{Schema, SchemaContext},
    };

    #[derive(Debug, Parser)]
    #[command(version, author, about)]
    struct Opts {
        /// Print more verbose logs
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,

        /// Compatibility check mode
        #[arg(long, default_value = "backward")]
        mode: Mode,

        /// File to compare against
        #[arg(long)]
        old: Option<PathBuf>,

        /// Input file
        file: PathBuf,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
    pub enum Mode {
        Forward,
        Backward,
        Both,
    }

    impl Mode {
        fn is_forward(self) -> bool { matches!(self, Self::Forward | Self::Both) }

        fn is_backward(self) -> bool { matches!(self, Self::Backward | Self::Both) }
    }

    #[inline]
    pub fn main() {
        let opts = Opts::parse();
        tracing::debug!("{opts:#?}");

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_file(false)
                    .with_line_number(false),
            )
            .with(match (cfg!(debug_assertions), opts.verbose) {
                (false, 0) => LevelFilter::INFO,
                (false, 1) | (true, 0) => LevelFilter::DEBUG,
                _ => LevelFilter::TRACE,
            })
            .init();

        std::process::exit(run(opts).map_or_else(
            |e| {
                tracing::error!("{e:?}");
                1
            },
            |()| 0,
        ));
    }

    #[inline]
    fn run(
        Opts {
            verbose: _,
            mode,
            old,
            file,
        }: Opts,
    ) -> Result<()> {
        let desc = protoc::get_descriptor_set([&file]).context("Error compiling proto file")?;
        let new_schema = Schema::new(&desc);
        let new_name = file.display().to_string();

        if let Some(old) = old {
            let old_name = old.display().to_string();
            check_protos(&new_schema, &new_name, old, &old_name, mode)?;
        } else {
            let repo = git::open().context("Error opening Git repository")?;

            let diffopt = git::diff_opts(&file);

            for commit in git::log(&repo, diffopt).context("Error getting file history")? {
                let (commit, id, blob) = commit
                    .and_then(|c| {
                        let id = git::commit_id(&c)?;
                        let blob = git::commit_file(&repo, &c, &file)?;
                        Ok((c, id, blob))
                    })
                    .context("Error reading file history")?;

                let Some(blob) = blob else { continue; };

                let _s = tracing::error_span!(
                    "check_commit",
                    hash = id.as_str(),
                    summary = commit.summary(),
                )
                .entered();
                tracing::debug!("Blob found, compiling and checking...");

                let mut tmp = tempfile::NamedTempFile::new()
                    .context("Error creating temporary proto file")?;
                tmp.write_all(blob.content())
                    .context("Error writing temporary proto file")?;

                let old_name = format!("{}:{}", id.as_str().unwrap_or_default(), file.display());

                check_protos(&new_schema, &new_name, tmp.path(), &old_name, mode)?;
            }
        }

        Ok(())
    }

    fn check_protos(
        new_schema: &Schema,
        new_name: &str,
        old: impl AsRef<std::ffi::OsStr>,
        old_name: &str,
        mode: Mode,
    ) -> Result<()> {
        let old_desc = protoc::get_descriptor_set([old])?;
        let old_schema = Schema::new(&old_desc);
        let mut res = Ok(());

        if mode.is_backward() {
            let ck = CompatPair::new(new_schema, &old_schema);
            let cx = CompatPair::new(SchemaContext { name: new_name }, SchemaContext {
                name: old_name,
            });
            let (reader, writer) = cx.as_ref().map(|c| c.name).into_inner();
            let _s = tracing::error_span!("check_backward", reader, writer).entered();
            res = res.and(CompatLog::run(
                |l| ck.check(cx, l),
                || {
                    tracing::error!(
                        "Backward-compatibility check of {new_name} against {old_name} failed"
                    );
                },
            ));
        }

        if mode.is_forward() {
            let ck = CompatPair::new(&old_schema, new_schema);
            let cx = CompatPair::new(SchemaContext { name: old_name }, SchemaContext {
                name: new_name,
            });
            let (reader, writer) = cx.as_ref().map(|c| c.name).into_inner();
            let _s = tracing::error_span!("check_forward", reader, writer).entered();
            res = res.and(CompatLog::run(
                |l| ck.check(cx, l),
                || {
                    tracing::error!(
                        "Forward-compatibility check of {new_name} against {old_name} failed"
                    );
                },
            ));
        }

        res.map_err(|()| anyhow::anyhow!("Stopping due to failed compatibility check"))
    }
}

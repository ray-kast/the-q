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

use std::{
    borrow::{Borrow, Cow},
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    hash::Hash,
    io::prelude::*,
    path::{Path, PathBuf},
    rc::Rc,
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use prost::Message as _;
use prost_types::FileDescriptorSet;
use tracing_subscriber::{filter::LevelFilter, prelude::*};

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
enum Mode {
    Forward,
    Backward,
    Both,
}

impl Mode {
    fn is_forward(self) -> bool { matches!(self, Self::Forward | Self::Both) }

    fn is_backward(self) -> bool { matches!(self, Self::Backward | Self::Both) }
}

fn main() {
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

    std::process::exit(run(opts).map_or(1, |()| 0));
}

fn run(
    Opts {
        verbose: _,
        mode,
        old,
        file,
    }: Opts,
) -> Result<()> {
    let desc = get_descriptor_set([&file]).context("Error compiling proto file")?;
    let new_schema = Schema::new(&desc);
    let new_name = file.display().to_string();

    if let Some(old) = old {
        let old_name = old.display().to_string();
        check_protos(&new_schema, &new_name, old, &old_name, mode)?;
    } else {
        let repo = git2::Repository::open_from_env().context("Error opening Git repository")?;

        let mut diffopt = git2::DiffOptions::new();
        diffopt.pathspec(&file);

        for commit in git_log(&repo, diffopt).context("Error getting file history")? {
            let (commit, id, blob) = commit
                .and_then(|commit| {
                    let id = commit.as_object().short_id()?;
                    let tree = commit.tree()?;

                    let entry = match tree.get_path(&file) {
                        Ok(e) => Some(e),
                        Err(e) if e.code() == git2::ErrorCode::NotFound => None,
                        Err(e) => return Err(e),
                    };

                    entry
                        .map(|e| e.to_object(&repo).and_then(|o| o.peel_to_blob()))
                        .transpose()
                        .map(|o| (commit, id, o))
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

            let mut tmp =
                tempfile::NamedTempFile::new().context("Error creating temporary proto file")?;
            tmp.write_all(blob.content())
                .context("Error writing temporary proto file")?;

            let old_name = format!("{}:{}", id.as_str().unwrap_or_default(), file.display());

            check_protos(&new_schema, &new_name, tmp.path(), &old_name, mode)?;
        }
    }

    Ok(())
}

fn git_log(
    repo: &'_ git2::Repository,
    mut diffopt: git2::DiffOptions,
) -> Result<impl Iterator<Item = Result<git2::Commit, git2::Error>> + '_, git2::Error> {
    let mut rw = repo.revwalk()?;
    rw.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    rw.push_head()?;

    Ok(std::iter::from_fn(move || {
        loop {
            let res = rw.next()?.and_then(|obj| {
                let commit = repo.find_commit(obj)?;
                let tree = commit.tree()?;

                let any_diff = if commit.parent_count() == 0 {
                    let diff = repo.diff_tree_to_tree(None, Some(&tree), Some(&mut diffopt))?;

                    diff.deltas().len() > 0
                } else {
                    let mut any = false;
                    for parent in commit.parents() {
                        let par_tree = parent.tree()?;
                        let diff = repo.diff_tree_to_tree(
                            Some(&par_tree),
                            Some(&tree),
                            Some(&mut diffopt),
                        )?;

                        if diff.deltas().len() > 0 {
                            any = true;
                            break;
                        }
                    }
                    any
                };

                Ok(any_diff.then_some(commit))
            });

            if let Some(res) = res.transpose() {
                break Some(res);
            }
        }
    }))
}

fn check_protos(
    new_schema: &Schema,
    new_name: &str,
    old: impl AsRef<std::ffi::OsStr> + AsRef<Path> + fmt::Debug,
    old_name: &str,
    mode: Mode,
) -> Result<()> {
    let old_desc = get_descriptor_set([old])?;
    let old_schema = Schema::new(&old_desc);

    if mode.is_backward() {
        let ck = CompatPair {
            reader: new_schema,
            writer: &old_schema,
        };
        let cx = CompatPair {
            reader: SchemaContext { name: new_name },
            writer: SchemaContext { name: old_name },
        };
        let _s = tracing::error_span!(
            "check_backward",
            reader = cx.reader.name,
            writer = cx.writer.name
        )
        .entered();
        ck.check(cx).map_err(|err| {
            tracing::error!("{err}");
            anyhow::anyhow!("Backward-compatibility check of {new_name} against {old_name} failed")
        })?;
    }

    if mode.is_forward() {
        let ck = CompatPair {
            reader: &old_schema,
            writer: new_schema,
        };
        let cx = CompatPair {
            reader: SchemaContext { name: old_name },
            writer: SchemaContext { name: new_name },
        };
        let _s = tracing::error_span!(
            "check_forward",
            reader = cx.reader.name,
            writer = cx.writer.name
        )
        .entered();
        ck.check(cx).map_err(|err| {
            tracing::error!("{err}");
            anyhow::anyhow!("Forward-compatibility check of {new_name} against {old_name} failed")
        })?;
    }

    Ok(())
}

fn get_descriptor_set<I: IntoIterator>(files: I) -> Result<FileDescriptorSet>
where I::Item: AsRef<std::ffi::OsStr> + AsRef<Path> {
    let mut tmp = tempfile::NamedTempFile::new().context("Error creating descriptor tempfile")?;

    let out = std::process::Command::new("protoc")
        .arg(format!("--descriptor_set_out={}", tmp.path().display()))
        .args(files)
        .output()
        .context("Error running protoc")?;
    for line in String::from_utf8_lossy(&out.stderr).lines() {
        if line.trim().is_empty() {
            continue;
        }

        tracing::warn!("{line}");
    }

    if !out.status.success() {
        bail!(
            "protoc exited with code {}",
            out.status.code().unwrap_or(-1)
        );
    }

    let mut bytes = vec![];
    tmp.as_file_mut()
        .read_to_end(&mut bytes)
        .context("Error reading descriptor set")?;

    FileDescriptorSet::decode(&*bytes).context("Error decoding descriptor set")
}

#[derive(PartialEq, Eq, Hash)]
struct QualName<'a> {
    package: Option<Cow<'a, str>>,
    path: Vec<Cow<'a, str>>,
}

impl<'a> fmt::Debug for QualName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(package) = &self.package {
            write!(f, "'{package}'")?;
        }

        for id in &self.path {
            write!(f, ".{id}")?;
        }

        Ok(())
    }
}

impl<'a> QualName<'a> {
    fn borrowed(&self) -> QualName<'_> {
        let Self { package, path } = self;

        QualName {
            package: package.as_ref().map(|p| p.as_ref().into()),
            path: path.iter().map(|p| p.as_ref().into()).collect(),
        }
    }

    fn to_owned(&self) -> QualName<'static> {
        let Self { package, path } = self;

        QualName {
            package: package.as_ref().map(|p| p.as_ref().to_owned().into()),
            path: path.iter().map(|p| p.as_ref().to_owned().into()).collect(),
        }
    }

    fn into_owned(self) -> QualName<'static> {
        let Self { package, path } = self;

        QualName {
            package: package.map(|p| p.into_owned().into()),
            path: path.into_iter().map(|p| p.into_owned().into()).collect(),
        }
    }

    fn member<'b>(&'b self, memb: impl Into<Cow<'b, str>>) -> MemberQualName<'b> {
        MemberQualName {
            ty: self.borrowed(),
            memb: memb.into(),
        }
    }
}

struct MemberQualName<'a> {
    ty: QualName<'a>,
    memb: Cow<'a, str>,
}

impl<'a> fmt::Debug for MemberQualName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}::{}", self.ty, self.memb)
    }
}

impl<'a> MemberQualName<'a> {
    fn borrowed(&self) -> MemberQualName<'_> {
        let Self { ty, memb } = self;

        MemberQualName {
            ty: ty.borrowed(),
            memb: memb.as_ref().into(),
        }
    }

    fn to_owned(&self) -> MemberQualName<'static> {
        let Self { ty, memb } = self;

        MemberQualName {
            ty: ty.to_owned(),
            memb: memb.as_ref().to_owned().into(),
        }
    }
}

trait CheckCompat {
    type Context<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult;
}

struct CompatError {
    reader_cx: Option<Box<dyn fmt::Debug>>,
    writer_cx: Option<Box<dyn fmt::Debug>>,
    message: String,
}

impl fmt::Display for CompatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.reader_cx, &self.writer_cx) {
            (None, None) => unreachable!(),
            (Some(r), None) => write!(f, "({r:?} in reader) "),
            (None, Some(w)) => write!(f, "({w:?} in writer) "),
            (Some(r), Some(w)) => write!(f, "({r:?} in reader, {w:?} in writer) "),
        }?;

        write!(f, "{}", self.message)
    }
}

impl CompatError {
    fn reader(cx: impl fmt::Debug + 'static, message: impl fmt::Display) -> Self {
        Self {
            reader_cx: Some(Box::new(cx)),
            writer_cx: None,
            message: message.to_string(),
        }
    }

    fn writer(cx: impl fmt::Debug + 'static, message: impl fmt::Display) -> Self {
        Self {
            reader_cx: None,
            writer_cx: Some(Box::new(cx)),
            message: message.to_string(),
        }
    }

    fn both(
        reader_cx: impl fmt::Debug + 'static,
        writer_cx: impl fmt::Debug + 'static,
        message: impl fmt::Display,
    ) -> Self {
        Self {
            reader_cx: Some(Box::new(reader_cx)),
            writer_cx: Some(Box::new(writer_cx)),
            message: message.to_string(),
        }
    }

    #[inline]
    fn warn(self) {
        tracing::warn!("{self}");
    }
}

type CompatResult = std::result::Result<(), CompatError>;

#[derive(Clone, Copy)]
struct CompatPair<T> {
    reader: T,
    writer: T,
}

impl From<()> for CompatPair<()> {
    fn from((): ()) -> Self {
        Self {
            reader: (),
            writer: (),
        }
    }
}

impl<'a, T: CheckCompat> CompatPair<&'a T> {
    #[inline]
    fn check(self, cx: CompatPair<T::Context<'_>>) -> CompatResult {
        CheckCompat::check_compat(self, cx)
    }
}

impl<'a, K: Eq + Hash, V> CompatPair<&'a HashMap<K, V>> {
    fn iter(self) -> impl Iterator<Item = (&'a K, CompatPair<Option<&'a V>>)> {
        let Self { reader, writer } = self;

        reader
            .iter()
            .map(|(key, reader)| {
                (key, CompatPair {
                    reader: Some(reader),
                    writer: writer.get(key),
                })
            })
            .chain(writer.iter().filter_map(|(key, writer)| {
                (!reader.contains_key(key)).then_some((key, CompatPair {
                    reader: None,
                    writer: Some(writer),
                }))
            }))
    }
}

impl<'a, K: Eq + Hash, V: CheckCompat> CompatPair<&'a HashMap<K, V>> {
    fn check_symmetric<'b, E>(
        self,
        extra: &'b CompatPair<E>,
        cx: impl Fn(&'b E, &'b K) -> V::Context<'b>,
        missing_read: impl Fn(&K, &V) -> CompatResult,
        missing_write: impl Fn(&K, &V) -> CompatResult,
    ) -> CompatResult
    where
        'a: 'b,
    {
        for (key, pair) in self.iter() {
            match (pair.reader, pair.writer) {
                (Some(reader), Some(writer)) => {
                    CompatPair { reader, writer }.check(CompatPair {
                        reader: cx(&extra.reader, key),
                        writer: cx(&extra.writer, key),
                    })?;
                },
                (Some(reader), None) => missing_write(key, reader)?,
                (None, Some(writer)) => missing_read(key, writer)?,
                (None, None) => unreachable!(),
            }
        }

        Ok(())
    }
}

type TypeMap = HashMap<QualName<'static>, Type>;

#[derive(Debug)]
struct Schema {
    types: TypeMap,
}

impl Schema {
    fn new(desc: &FileDescriptorSet) -> Self {
        let mut me = Self {
            types: HashMap::new(),
        };

        Visitor(&mut me).fildes_set(desc);

        tracing::trace!("{me:#?}");

        me
    }
}

struct SchemaContext<'a> {
    name: &'a str,
}

impl CheckCompat for Schema {
    type Context<'a> = SchemaContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Schema>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;
        let CompatPair {
            reader: rd_name,
            writer: wr_name,
        } = cx;

        let Self { types: rd_types } = reader;
        let Self { types: wr_types } = writer;

        CompatPair {
            reader: rd_types,
            writer: wr_types,
        }
        .check_symmetric(
            &CompatPair {
                reader: rd_types,
                writer: wr_types,
            },
            |types, name| TypeContext {
                kind: TypeCheckKind::ByName(name.borrowed()),
                types,
            },
            |wk, wv| {
                if wv.is_internal() {
                    Ok(())
                } else {
                    Err(CompatError::both(
                        rd_name.name.to_owned(),
                        wr_name.name.to_owned(),
                        format!("Missing {} type {wk:?} present in writer", wv.var()),
                    ))
                }
            },
            |_, _| Ok(()),
        )
    }
}

enum ReservedMap {
    Reserved(BTreeMap<i64, bool>),
    Deprecated,
}

impl fmt::Debug for ReservedMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("ReservedMap");

        match self {
            Self::Reserved(res) => {
                let mut start = None;
                for (idx, enabled) in res {
                    if *enabled {
                        assert!(std::mem::replace(&mut start, Some(idx)).is_none());
                    } else if let Some(start) = start.take() {
                        d.field(&(start..idx));
                    } else {
                        d.field(&(..idx));
                    }
                }

                if let Some(start) = start {
                    d.field(&(start..));
                }
            },
            Self::Deprecated => {
                d.field(&(..));
            },
        }

        d.finish()
    }
}

impl ReservedMap {
    fn build_reserved(it: impl IntoIterator<Item = std::ops::Range<i64>>) -> Self {
        let mut map: BTreeMap<i64, bool> = BTreeMap::new();
        let mut over = vec![];

        for range in it {
            tracing::trace!(?map, ?range, "About to check overlap");
            let start = range.start;
            let end = range.end;

            debug_assert!(over.is_empty());
            over.extend(map.range(start..=end).map(|(i, e)| (*i, *e)));

            let start = over.first().map_or(true, |(_, e)| *e).then_some(start);
            let end = over.last().map_or(true, |(_, e)| !e).then_some(end);

            tracing::trace!(?over, ?start, ?end, "Overlap check done");
            debug_assert!(over.len() % 2 == usize::from(start.is_some() != end.is_some()));

            for (idx, _) in over.drain(..) {
                assert!(map.remove(&idx).is_some());
            }

            if let Some(start) = start {
                assert!(map.insert(start, true).is_none());
            }

            if let Some(end) = end {
                assert!(map.insert(end, false).is_none());
            }
        }

        Self::Reserved(map)
    }

    fn contains(&self, val: i64) -> bool {
        use std::ops::Bound;

        let map = match self {
            Self::Reserved(m) => m,
            Self::Deprecated => return true,
        };

        let left = map.range(..=val).next_back();
        let right = map.range((Bound::Excluded(val), Bound::Unbounded)).next();

        debug_assert!(left.map_or(true, |(i, _)| *i <= val));
        debug_assert!(right.map_or(true, |(i, _)| *i > val));

        match (left, right) {
            (None, None) => {
                debug_assert!(map.is_empty());
                false
            },
            (None, Some((_, &next_start))) => !next_start,
            (Some((_, &prev_end)), None) => prev_end,
            (Some((_, &prev_end)), Some((_, &next_start))) => {
                assert!(prev_end != next_start);
                prev_end
            },
        }
    }
}

#[derive(Debug)]
struct UnionFindNode {
    parent: usize,
    rank: usize,
}

#[derive(Debug, Default)]
struct UnionFind(Vec<UnionFindNode>);

impl UnionFind {
    fn put(&mut self) -> usize {
        let key = self.0.len();
        self.0.push(UnionFindNode {
            parent: key,
            rank: 1,
        });
        key
    }

    fn find(&mut self, key: usize) -> Option<usize> {
        let entry = self.0.get(key)?;

        if entry.parent == key {
            Some(entry.parent)
        } else {
            let root = self.find(entry.parent).unwrap();

            debug_assert!(self.0.len() > key);
            // Safety: find does not change the element count
            unsafe { self.0.get_unchecked_mut(key).parent = root };

            Some(root)
        }
    }

    fn union(&mut self, a: usize, b: usize) -> Result<Option<usize>, ()> {
        use std::cmp::Ordering;

        let mut a = self.find(a).ok_or(())?;
        let mut b = self.find(b).ok_or(())?;

        let mut a_rank;
        let mut b_rank;
        debug_assert!(self.0.len() > a);
        debug_assert!(self.0.len() > b);
        // Safety: find does not change the element count
        unsafe {
            a_rank = self.0.get_unchecked(a).rank;
            b_rank = self.0.get_unchecked(b).rank;
        }

        match a.cmp(&b) {
            Ordering::Equal => return Ok(None),
            Ordering::Greater if a_rank <= b_rank => {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut a_rank, &mut b_rank);
            },
            Ordering::Less | Ordering::Greater => (),
        }

        debug_assert!((a_rank, b) > (b_rank, a));

        // Safety: find nor any operations since the last unsafe block do not
        //         change the element count or key values
        unsafe {
            self.0.get_unchecked_mut(a).rank += b_rank;
            debug_assert!(self.0[a].rank == a_rank + b_rank);
            self.0.get_unchecked_mut(b).parent = a;
        }

        Ok(Some(a))
    }
}

#[derive(Debug)]
enum Type {
    Message(Record<Field>),
    Enum(Record<Variant>),
}

impl Type {
    fn var(&self) -> &'static str {
        match self {
            Type::Message(_) => "message",
            Type::Enum(_) => "enum",
        }
    }

    fn is_internal(&self) -> bool {
        match self {
            Self::Message(m) => m.internal,
            Self::Enum(e) => e.internal,
        }
    }
}

enum TypeCheckKind<'a> {
    ByName(QualName<'a>),
    ForField {
        name: MemberQualName<'a>,
        ty: QualName<'a>,
    },
}

impl<'a> fmt::Debug for TypeCheckKind<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByName(q) => write!(f, "{q:?}"),
            Self::ForField { name, ty } => write!(f, "{name:?}::<{ty:?}>"),
        }
    }
}

impl<'a> TypeCheckKind<'a> {
    fn to_owned(&self) -> TypeCheckKind<'static> {
        match self {
            Self::ByName(q) => TypeCheckKind::ByName(q.to_owned()),
            Self::ForField { name, ty } => TypeCheckKind::ForField {
                name: name.to_owned(),
                ty: ty.to_owned(),
            },
        }
    }
}

impl<'a> TypeCheckKind<'a> {
    fn ty(&self) -> &QualName<'a> {
        match self {
            Self::ByName(q) => q,
            Self::ForField { ty, .. } => ty,
        }
    }
}

struct TypeContext<'a> {
    kind: TypeCheckKind<'a>,
    types: &'a TypeMap,
}

impl CheckCompat for Type {
    type Context<'a> = TypeContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Type>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;
        match (reader, writer) {
            (Self::Message(reader), Self::Message(writer)) => {
                CompatPair { reader, writer }.check(cx)
            },
            (Self::Enum(reader), Self::Enum(writer)) => CompatPair { reader, writer }.check(cx),
            (rd, wr) => Err(CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!(
                    "Type mismatch: {} in reader, {} in writer",
                    rd.var(),
                    wr.var()
                ),
            )),
        }
    }
}

struct RecordContext<'a> {
    ty: &'a TypeContext<'a>,
    id: i32,
}

trait RecordValue<'a>: CheckCompat<Context<'a> = RecordContext<'a>> {
    type Names: Iterator<Item = &'a str> + ExactSizeIterator;

    fn names(&'a self) -> Self::Names;

    fn missing_reader_id(
        &self,
        cx: &CompatPair<TypeContext<'a>>,
        wr_id: i32,
        reserved: impl FnOnce() -> bool,
    ) -> CompatResult;

    fn missing_writer_id(
        &self,
        cx: &CompatPair<TypeContext<'a>>,
        rd_id: i32,
        reserved: impl FnOnce() -> bool,
    ) -> CompatResult;

    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
    ) -> CompatResult;

    fn missing_reader_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        wr_id: Option<i32>,
    ) -> CompatResult;

    fn missing_writer_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        rd_id: Option<i32>,
    ) -> CompatResult;

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
    ) -> CompatResult
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Sided<T> {
    Read(T),
    Write(T),
}

impl<T> Sided<T> {
    fn then<U>(&self, val: U) -> Sided<U> {
        match self {
            Self::Read(_) => Sided::Read(val),
            Self::Write(_) => Sided::Write(val),
        }
    }

    fn inner(self) -> T {
        match self {
            Self::Read(v) | Self::Write(v) => v,
        }
    }
}

impl<'a> RecordValue<'a> for Field {
    type Names = std::iter::Once<&'a str>;

    fn names(&'a self) -> Self::Names { std::iter::once(&self.name) }

    fn missing_reader_id(
        &self,
        cx: &CompatPair<TypeContext<'a>>,
        wr_id: i32,
        reserved: impl FnOnce() -> bool,
    ) -> CompatResult {
        if !reserved() {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!(
                    "Field {} (ID {wr_id}) missing and not reserved on reader",
                    self.name
                ),
            )
            .warn();
        }

        Ok(())
    }

    fn missing_writer_id(
        &self,
        cx: &CompatPair<TypeContext<'a>>,
        rd_id: i32,
        reserved: impl FnOnce() -> bool,
    ) -> CompatResult {
        if !reserved() {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!(
                    "Field {} (ID {rd_id}) missing and not reserved on writer",
                    self.name
                ),
            )
            .warn();
        }

        Ok(())
    }

    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
    ) -> CompatResult {
        CompatError::both(
            cx.reader.kind.to_owned(),
            cx.writer.kind.to_owned(),
            format!(
                "Field {name} has id {} on reader and {} on writer",
                ids.reader, ids.writer
            ),
        )
        .warn();
        Ok(())
    }

    fn missing_reader_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        wr_id: Option<i32>,
    ) -> CompatResult {
        if let (TypeCheckKind::ByName(_), Some(id)) = (&cx.reader.kind, wr_id) {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!("Field name {name} (ID {id}) missing and not reserved on reader"),
            )
            .warn();
        }

        Ok(())
    }

    fn missing_writer_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        rd_id: Option<i32>,
    ) -> CompatResult {
        if let (TypeCheckKind::ByName(_), Some(id)) = (&cx.reader.kind, rd_id) {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!("Field name {name} (ID {id}) missing and not reserved on writer"),
            )
            .warn();
        }

        Ok(())
    }

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
    ) -> CompatResult
    where
        Self: Sized,
    {
        use std::collections::hash_map::Entry;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum Group {
            Uniq(usize),
            Oneof(i32),
        }

        #[derive(Debug, PartialEq, Eq, Hash)]
        struct FieldInfo<'a> {
            name: &'a str,
            group: Group,
        }

        let mut uf_ids: HashMap<i32, usize> = HashMap::new();
        let mut fields: HashMap<usize, HashSet<Sided<FieldInfo>>> = HashMap::new();
        let mut group_reps: HashMap<Sided<Group>, usize> = HashMap::new();
        let mut uf: UnionFind = UnionFind::default();
        let mut next_uniq = 0_usize;

        for ((key, val), side) in ck
            .reader
            .zip(std::iter::repeat(Sided::Read(())))
            .chain(ck.writer.zip(std::iter::repeat(Sided::Write(()))))
        {
            let group = val.oneof.map_or_else(
                || {
                    let next = next_uniq + 1;
                    Group::Uniq(std::mem::replace(&mut next_uniq, next))
                },
                Group::Oneof,
            );

            let uf_id = match uf_ids.entry(*key) {
                Entry::Occupied(o) => *o.get(),
                Entry::Vacant(v) => {
                    let uf_id = uf.put();
                    v.insert(uf_id);
                    uf_id
                },
            };

            let field = FieldInfo {
                name: &val.name,
                group,
            };

            assert!(fields.entry(uf_id).or_default().insert(side.then(field)));

            if let Some(prev) = group_reps.insert(side.then(group), uf_id) {
                assert!(!matches!(group, Group::Uniq(_)));
                uf.union(prev, uf_id).unwrap();
            }
        }

        let mut clashes: HashMap<Sided<usize>, BTreeSet<usize>> = HashMap::new();

        for &uf_id in uf_ids.values() {
            let fields = fields.get(&uf_id).unwrap();
            let root = uf.find(uf_id).unwrap();

            for field in fields {
                clashes.entry(field.then(root)).or_default().insert(uf_id);
            }
        }

        let clashes_rev: HashMap<BTreeSet<usize>, HashSet<Sided<usize>>> = clashes
            .into_iter()
            .fold(HashMap::default(), |mut map, (k, v)| {
                map.entry(v).or_default().insert(k);
                map
            });

        for (clash, rep) in clashes_rev {
            if clash.len() < 2 {
                continue;
            }

            let clash_fields: HashSet<&Sided<FieldInfo>> =
                clash.iter().flat_map(|k| fields.get(k).unwrap()).collect();
            let rep_fields: HashSet<&Sided<FieldInfo>> = rep
                .iter()
                .flat_map(|v| fields.get(&v.inner()).unwrap())
                .collect();
            // TODO: locate relevant groups

            let mut s = "Oneof group clash - fields involved: ".to_owned();

            for (i, field) in clash_fields.iter().enumerate() {
                use std::fmt::Write;

                if i != 0 {
                    s.push_str(", ");
                }

                match field {
                    Sided::Read(r) => write!(s, "{} on reader", r.name),
                    Sided::Write(w) => write!(s, "{} on writer", w.name),
                }
                .unwrap();
            }

            // TODO: continue on non-fatal errors
            return Err(CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                s,
            ));
        }

        Ok(())
    }
}

impl<'a> RecordValue<'a> for Variant {
    type Names = std::iter::Map<std::collections::btree_set::Iter<'a, String>, fn(&String) -> &str>;

    fn names(&'a self) -> Self::Names { self.0.iter().map(AsRef::as_ref) }

    fn missing_reader_id(
        &self,
        cx: &CompatPair<TypeContext<'a>>,
        wr_id: i32,
        reserved: impl FnOnce() -> bool,
    ) -> CompatResult {
        if !reserved() {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!(
                    "Enum variant(s) {} (value {wr_id}) missing and not reserved on reader",
                    self.name_pretty(false)
                ),
            )
            .warn();
        }

        Ok(())
    }

    fn missing_writer_id(
        &self,
        cx: &CompatPair<TypeContext<'a>>,
        rd_id: i32,
        reserved: impl FnOnce() -> bool,
    ) -> CompatResult {
        if !reserved() {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!(
                    "Enum variant(s) {} (value {rd_id}) missing and not reserved on writer",
                    self.name_pretty(false),
                ),
            )
            .warn();
        }

        Ok(())
    }

    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
    ) -> CompatResult {
        Err(CompatError::both(
            cx.reader.kind.to_owned(),
            cx.writer.kind.to_owned(),
            format!(
                "Enum variant {name} has value {} on reader and {} on writer",
                ids.reader, ids.writer
            ),
        ))
    }

    fn missing_reader_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        wr_id: Option<i32>,
    ) -> CompatResult {
        if let Some(id) = wr_id {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!("Enum variant name {name} (ID {id}) missing and not reserved on reader"),
            )
            .warn();
        }

        Ok(())
    }

    fn missing_writer_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        rd_id: Option<i32>,
    ) -> CompatResult {
        if let Some(id) = rd_id {
            CompatError::both(
                cx.reader.kind.to_owned(),
                cx.writer.kind.to_owned(),
                format!("Enum variant name {name} (ID {id}) missing and not reserved on writer"),
            )
            .warn();
        }

        Ok(())
    }

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
    ) -> CompatResult
    where
        Self: Sized,
    {
        let CompatPair { reader, writer } = ck;

        for (val, var) in reader {
            if *val < 0 {
                CompatError::reader(
                    cx.reader.kind.ty().member(var.name_pretty(true)).to_owned(),
                    format!("Negative enum value {val}"),
                )
                .warn();
            }
        }

        for (val, var) in writer {
            if *val < 0 {
                CompatError::writer(
                    cx.writer.kind.ty().member(var.name_pretty(true)).to_owned(),
                    format!("Negative enum value {val}"),
                )
                .warn();
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct Record<T> {
    numbers: HashMap<i32, T>,
    /// `None` indicates a reserved name
    names: HashMap<String, Option<i32>>,
    reserved: ReservedMap,
    internal: bool,
}

impl<T: for<'a> RecordValue<'a>> CheckCompat for Record<T> {
    type Context<'a> = TypeContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        let Self {
            numbers: rd_nums,
            names: rd_names,
            reserved: rd_res,
            internal: _,
        } = reader;
        let Self {
            numbers: wr_nums,
            names: wr_names,
            reserved: wr_res,
            internal: _,
        } = writer;

        CompatPair {
            reader: rd_nums,
            writer: wr_nums,
        }
        .check_symmetric(
            &cx,
            |ty, &id| RecordContext { ty, id },
            |wk, wv| wv.missing_reader_id(&cx, *wk, || rd_res.contains((*wk).into())),
            |rk, rv| rv.missing_writer_id(&cx, *rk, || wr_res.contains((*rk).into())),
        )?;

        let names = CompatPair {
            reader: rd_names,
            writer: wr_names,
        };

        for (key, pair) in names.iter() {
            match (pair.reader, pair.writer) {
                (Some(&rd_id), Some(&wr_id)) => match (rd_id, wr_id) {
                    (Some(reader), Some(writer)) if reader != writer => {
                        T::id_conflict(&cx, key, CompatPair { reader, writer })
                    },
                    (..) => Ok(()),
                },
                (Some(&r), None) => T::missing_writer_name(&cx, key, r),
                (None, Some(&w)) => T::missing_reader_name(&cx, key, w),
                (None, None) => unreachable!(),
            }?;
        }

        T::check_extra(
            CompatPair {
                reader: rd_nums.iter(),
                writer: wr_nums.iter(),
            },
            &cx,
        )
    }
}

impl<T: for<'a> RecordValue<'a>> Record<T> {
    fn new<R: IntoIterator<Item = String>>(
        numbers: HashMap<i32, T>,
        reserved: ReservedMap,
        reserved_names: R,
        internal: bool,
    ) -> Self
    where
        R::IntoIter: ExactSizeIterator,
    {
        let reserved_names = reserved_names.into_iter();
        let reserved_name_len = reserved_names.len();
        let names: HashMap<_, _> = numbers
            .iter()
            .flat_map(|(i, v)| v.names().zip(std::iter::repeat(*i)))
            .map(|(v, i)| (v.into(), Some(i)))
            .chain(reserved_names.map(|r| (r, None)))
            .collect();

        assert_eq!(
            names.len(),
            numbers.values().map(|v| v.names().len()).sum::<usize>() + reserved_name_len
        );

        Self {
            numbers,
            names,
            reserved,
            internal,
        }
    }
}

#[derive(Debug)]
struct Field {
    name: String,
    ty: FieldType,
    kind: FieldKind,
    oneof: Option<i32>,
}

impl CheckCompat for Field {
    type Context<'a> = RecordContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        let Self {
            name: rd_name,
            ty: rd_ty,
            kind: rd_kind,
            oneof: _,
        } = reader;
        let Self {
            name: wr_name,
            ty: wr_ty,
            kind: wr_kind,
            oneof: _,
        } = writer;

        assert_eq!(cx.reader.id, cx.writer.id);
        let id = cx.reader.id;

        let rd_qual_name = cx.reader.ty.kind.ty().member(rd_name);
        let wr_qual_name = cx.writer.ty.kind.ty().member(wr_name);

        let cx = CompatPair {
            reader: FieldContext {
                name: rd_qual_name.borrowed(),
                types: cx.reader.ty.types,
                kind: *rd_kind,
            },
            writer: FieldContext {
                name: wr_qual_name.borrowed(),
                types: cx.writer.ty.types,
                kind: *wr_kind,
            },
        };

        if rd_name != wr_name {
            CompatError::both(
                cx.reader.name.to_owned(),
                cx.writer.name.to_owned(),
                format!("Field name mismatch for ID {id}"),
            )
            .warn();
        }

        CompatPair {
            reader: rd_ty,
            writer: wr_ty,
        }
        .check(cx)?;

        CompatPair {
            reader: rd_kind,
            writer: wr_kind,
        }
        .check(CompatPair {
            reader: rd_qual_name,
            writer: wr_qual_name,
        })?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VarIntMode {
    Signed,
    Unsigned,
    ZigZag,
    Enum,
}

impl CheckCompat for VarIntMode {
    type Context<'a> = FieldContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        match (*reader, *writer) {
            (a, b) if a == b => Ok(()),
            (rd @ (Self::Signed | Self::Unsigned), wr @ (Self::Signed | Self::Unsigned)) => {
                CompatError::both(
                    cx.reader.name.to_owned(),
                    cx.writer.name.to_owned(),
                    format!("Varint sign difference ({rd:?} in reader, {wr:?} in writer)"),
                )
                .warn();
                Ok(())
            },
            (rd @ (Self::Signed | Self::Unsigned), wr @ Self::Enum)
            | (rd @ Self::Enum, wr @ (Self::Signed | Self::Unsigned)) => {
                CompatError::both(
                    cx.reader.name.to_owned(),
                    cx.writer.name.to_owned(),
                    format!("Enum type punning ({rd:?} in reader, {wr:?} in writer)"),
                )
                .warn();
                Ok(())
            },
            (rd, wr) => Err(CompatError::both(
                cx.reader.name.to_owned(),
                cx.writer.name.to_owned(),
                format!("Incompatible varint formats ({rd:?} in reader, {wr:?} in writer)"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixIntMode {
    Signed,
    Unsigned,
    Float,
}

impl CheckCompat for FixIntMode {
    type Context<'a> = FieldContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;
        match (*reader, *writer) {
            (a, b) if a == b => Ok(()),
            (rd @ (Self::Signed | Self::Unsigned), wr @ (Self::Signed | Self::Unsigned)) => {
                CompatError::both(
                    cx.reader.name.to_owned(),
                    cx.writer.name.to_owned(),
                    format!(
                        "Sign difference in fixint fields ({rd:?} in reader, {wr:?} in writer)"
                    ),
                )
                .warn();

                Ok(())
            },
            (rd, wr) => Err(CompatError::both(
                cx.reader.name.to_owned(),
                cx.writer.name.to_owned(),
                format!("Incompatible fixint formats ({rd:?} in reader, {wr:?} in writer)"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BytesMode {
    Bytes,
    Utf8,
    Message,
    Packed(NumericWireType),
}

impl CheckCompat for BytesMode {
    type Context<'a> = FieldContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        match (*reader, *writer) {
            (a, b) if a == b => Ok(()),
            (rd @ (Self::Bytes | Self::Utf8), wr @ (Self::Bytes | Self::Utf8)) => {
                CompatError::both(
                    cx.reader.name.to_owned(),
                    cx.writer.name.to_owned(),
                    format!("UTF-8 type punning ({rd:?} in reader, {wr:?} in writer)"),
                )
                .warn();
                Ok(())
            },
            (rd @ (Self::Bytes | Self::Message), wr @ (Self::Bytes | Self::Message)) => {
                CompatError::both(
                    cx.reader.name.to_owned(),
                    cx.writer.name.to_owned(),
                    format!("Embedded message type punning ({rd:?} in reader, {wr:?} in writer)"),
                )
                .warn();
                Ok(())
            },
            (rd, wr) => Err(CompatError::both(
                cx.reader.name.to_owned(),
                cx.writer.name.to_owned(),
                format!("Incompatible byte formats ({rd:?} in reader, {wr:?} in writer)"),
            )),
        }
    }
}

type NumericWireType = WireType<std::convert::Infallible>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireType<B = BytesMode> {
    VarInt(VarIntMode),
    Fix32(FixIntMode),
    Fix64(FixIntMode),
    Bytes(B),
}

impl WireType {
    fn to_numeric(self) -> Option<NumericWireType> {
        match self {
            Self::VarInt(m) => Some(WireType::VarInt(m)),
            Self::Fix32(m) => Some(WireType::Fix32(m)),
            Self::Fix64(m) => Some(WireType::Fix64(m)),
            Self::Bytes(_) => None,
        }
    }

    fn adjust_for_kind(self, kind: FieldKind) -> Self {
        match (self.to_numeric(), kind) {
            (
                Some(n),
                FieldKind::Repeated {
                    packed: None | Some(true),
                },
            ) => Self::Bytes(BytesMode::Packed(n)),
            (..) => self,
        }
    }
}

// TODO: warn on non-zigzag types and negative enums?
#[derive(Debug, Clone, Copy)]
enum PrimitiveType {
    F64,    // Double
    F32,    // Float
    VarI64, // Int64
    VarU64, // Uint64
    VarI32, // Int32
    FixU64, // Fixed64
    FixU32, // Fixed32
    Bool,   // Bool
    String, // String
    Bytes,  // Bytes
    VarU32, // Uint32
    FixI32, // Sfixed32
    FixI64, // Sfixed64
    VarZ32, // Sint32
    VarZ64, // Sint64
}

impl PrimitiveType {
    fn wire_format(self, kind: FieldKind) -> WireType {
        match self {
            Self::F64 => WireType::Fix64(FixIntMode::Float),
            Self::F32 => WireType::Fix32(FixIntMode::Float),
            Self::VarI64 | Self::VarI32 => WireType::VarInt(VarIntMode::Signed),
            Self::VarU64 | Self::VarU32 | Self::Bool => WireType::VarInt(VarIntMode::Unsigned),
            Self::FixU64 => WireType::Fix64(FixIntMode::Unsigned),
            Self::FixU32 => WireType::Fix32(FixIntMode::Unsigned),
            Self::String => WireType::Bytes(BytesMode::Utf8),
            Self::Bytes => WireType::Bytes(BytesMode::Bytes),
            Self::FixI32 => WireType::Fix32(FixIntMode::Signed),
            Self::FixI64 => WireType::Fix64(FixIntMode::Signed),
            Self::VarZ32 | Self::VarZ64 => WireType::VarInt(VarIntMode::ZigZag),
        }
        .adjust_for_kind(kind)
    }
}

impl PrimitiveType {
    fn build(ty: prost_types::field_descriptor_proto::Type) -> Option<Self> {
        use prost_types::field_descriptor_proto::Type;

        Some(match ty {
            Type::Double => Self::F64,
            Type::Float => Self::F32,
            Type::Int64 => Self::VarI64,
            Type::Uint64 => Self::VarU64,
            Type::Int32 => Self::VarI32,
            Type::Fixed64 => Self::FixU64,
            Type::Fixed32 => Self::FixU32,
            Type::Bool => Self::Bool,
            Type::String => Self::String,
            Type::Bytes => Self::Bytes,
            Type::Uint32 => Self::VarU32,
            Type::Sfixed32 => Self::FixI32,
            Type::Sfixed64 => Self::FixI64,
            Type::Sint32 => Self::VarZ32,
            Type::Sint64 => Self::VarZ64,
            Type::Group | Type::Message | Type::Enum => return None,
        })
    }
}

struct FieldContext<'a> {
    name: MemberQualName<'a>,
    types: &'a TypeMap,
    kind: FieldKind,
}

#[derive(Debug)]
enum FieldType {
    Primitive(PrimitiveType),
    Named(QualName<'static>),
}

impl CheckCompat for FieldType {
    type Context<'a> = FieldContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        let rd_name = cx.reader.name.to_owned();
        let wr_name = cx.writer.name.to_owned();
        let rd_types = cx.reader.types;
        let wr_types = cx.writer.types;

        let rd_wire = reader.wire_format(cx.reader.kind, |n| rd_types.get(n).unwrap());
        let wr_wire = writer.wire_format(cx.writer.kind, |n| wr_types.get(n).unwrap());

        match (rd_wire, wr_wire) {
            (WireType::VarInt(ref reader), WireType::VarInt(ref writer)) => {
                CompatPair { reader, writer }.check(cx)
            },
            (WireType::Fix32(ref reader), WireType::Fix32(ref writer))
            | (WireType::Fix64(ref reader), WireType::Fix64(ref writer)) => {
                CompatPair { reader, writer }.check(cx)
            },
            (WireType::Bytes(ref reader), WireType::Bytes(ref writer)) => {
                CompatPair { reader, writer }.check(cx)
            },
            (rd, wr) => Err(CompatError::both(
                cx.reader.name.to_owned(),
                cx.writer.name.to_owned(),
                format!(
                    "Fields have incompatible wire formats ({rd:?} for reader, {wr:?} for writer)"
                ),
            )),
        }?;

        if let (Self::Named(reader), Self::Named(writer)) = (reader, writer) {
            assert_eq!(rd_wire, wr_wire);

            let rd_ty = rd_types.get(reader).unwrap();
            let wr_ty = wr_types.get(writer).unwrap();

            let cx = CompatPair {
                reader: TypeContext {
                    kind: TypeCheckKind::ForField {
                        name: rd_name,
                        ty: reader.borrowed(),
                    },
                    types: rd_types,
                },
                writer: TypeContext {
                    kind: TypeCheckKind::ForField {
                        name: wr_name,
                        ty: writer.borrowed(),
                    },
                    types: wr_types,
                },
            };

            CompatPair {
                reader: rd_ty,
                writer: wr_ty,
            }
            .check(cx)?;
        }

        Ok(())
    }
}

impl FieldType {
    fn wire_format<'a>(
        &'a self,
        kind: FieldKind,
        ty: impl Fn(&'a QualName<'a>) -> &'a Type,
    ) -> WireType {
        match self {
            &Self::Primitive(p) => p.wire_format(kind),
            Self::Named(n) => match ty(n) {
                Type::Message(_) => WireType::Bytes(BytesMode::Message),
                Type::Enum(_) => WireType::VarInt(VarIntMode::Enum),
            }
            .adjust_for_kind(kind),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Singular,
    Repeated { packed: Option<bool> },
    Optional,
}

impl CheckCompat for FieldKind {
    type Context<'a> = MemberQualName<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        match (*reader, *writer) {
            (a, b) if a == b => Ok(()),
            (Self::Singular | Self::Optional, Self::Singular | Self::Optional) => Ok(()),
            (Self::Repeated { packed }, Self::Singular | Self::Optional) => {
                assert!(!matches!(packed, Some(true)));
                Ok(())
            },
            (rd @ (Self::Singular | Self::Optional), Self::Repeated { packed }) => {
                assert!(!matches!(packed, Some(true)));
                CompatError::both(
                    cx.reader.to_owned(),
                    cx.writer.to_owned(),
                    format!("Repeated/singular mismatch ({rd:?} on reader, repeated on writer)"),
                )
                .warn();
                Ok(())
            },
            (rd, wr) => Err(CompatError::both(
                cx.reader.to_owned(),
                cx.writer.to_owned(),
                format!("Incompatible field kinds ({rd:?} on reader, {wr:?} on writer)"),
            )),
        }
    }
}

impl FieldKind {
    fn build(
        label: prost_types::field_descriptor_proto::Label,
        packed: Option<bool>,
        proto3_optional: Option<bool>,
    ) -> Self {
        use prost_types::field_descriptor_proto::Label;

        if !matches!(label, Label::Repeated) {
            assert!(packed.is_none());
        }

        match (label, proto3_optional) {
            (Label::Optional, Some(false) | None) => Self::Singular,
            (Label::Required, None) => panic!("Unsupported required label found"),
            (Label::Repeated, None) => Self::Repeated { packed },
            (Label::Optional, Some(true)) => Self::Optional,
            (l, o) => panic!("Unexpected field kind ({l:?}, optional={o:?})"),
        }
    }
}

#[derive(Debug, Default)]
#[repr(transparent)]
struct Variant(BTreeSet<String>);

impl CheckCompat for Variant {
    type Context<'a> = RecordContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let CompatPair { reader, writer } = ck;

        let qual_name = CompatPair {
            reader: cx.reader.ty.kind.ty().member(reader.name_pretty(true)),
            writer: cx.writer.ty.kind.ty().member(writer.name_pretty(true)),
        };

        let Self(reader) = reader;
        let Self(writer) = writer;

        assert_eq!(cx.reader.id, cx.writer.id);
        let id = cx.reader.id;

        if reader != writer {
            match (reader.len(), writer.len()) {
                (0, _) | (_, 0) => unreachable!(),
                (1, 1) => {
                    CompatError::both(
                        qual_name.reader.to_owned(),
                        qual_name.writer.to_owned(),
                        format!("Enum variant name mismatch for value {id}"),
                    )
                    .warn();
                },
                (..) => {
                    let mut rd_only = reader.difference(writer).peekable();
                    let mut wr_only = writer.difference(reader).peekable();

                    if rd_only.peek().is_some() && wr_only.peek().is_some() {
                        let mut s = format!("Mismatched enum alias(es) for value {id}");
                        let mut any = false;

                        for name in rd_only {
                            if any {
                                s.push_str(", ");
                            } else {
                                any = true;
                                s.push_str(": ");
                            }

                            s.push_str(name);
                        }

                        if any {
                            s.push_str(" for reader");
                        }

                        let prev_any = any;
                        let mut any = false;

                        for name in wr_only {
                            if any {
                                s.push_str(", ");
                            } else {
                                any = true;
                                s.push_str(if prev_any { "; " } else { ": " });
                            }

                            s.push_str(name);
                        }

                        if any {
                            s.push_str(" for writer");
                        }

                        CompatError::both(
                            qual_name.reader.to_owned(),
                            qual_name.writer.to_owned(),
                            s,
                        )
                        .warn();
                    }
                },
            }
        }

        Ok(())
    }
}

impl Variant {
    fn name_pretty(&self, compact: bool) -> Cow<'_, str> {
        let mut it = self.0.iter();
        let mut ret = Cow::Borrowed(&**it.next().unwrap());

        for part in it {
            let s = ret.to_mut();
            s.push_str(if compact { "|" } else { ", " });
            s.push_str(part);
        }

        ret
    }
}

#[derive(Debug)]
struct GlobalScope<'a> {
    packages: HashMap<Option<&'a str>, Scope<'a>>,
}

impl<'a> GlobalScope<'a> {
    fn build(fildes_set: &'a FileDescriptorSet) -> Self {
        Self {
            packages: fildes_set
                .file
                .iter()
                .map(|f| (f.package.as_deref(), Scope::build_package(f)))
                .collect(),
        }
    }

    fn package<Q: Eq + Hash + ?Sized>(&'a self, package: &Q) -> Option<ScopeRef<'a>>
    where Option<&'a str>: Borrow<Q> {
        self.packages.get(package).map(|scope| {
            assert!(matches!(scope, Scope::Package { .. }));
            ScopeRef {
                global: self,
                parent: None,
                scope,
            }
        })
    }

    fn resolve_one(&'a self, name: &'a str) -> Option<(Option<&'a str>, ScopeRef<'a>)> {
        let package = self.packages.get(&Some(name));
        let anon = self
            .packages
            .get(&None)
            .and_then(|p| p.items().get(name).map(|c| (p, c)));

        let (package, scope) = match (package, anon) {
            (None, None) => return None,
            (Some(pkg), None) => (pkg, pkg),
            (None, Some((pkg, scope))) => (pkg, scope),
            (Some(_), Some(_)) => {
                panic!("Conflict for {name:?} between package and anon-packaged type")
            },
        };

        let Scope::Package { name, .. } = *package else { panic!("Invalid global scope") };

        Some((name, ScopeRef {
            global: self,
            parent: None,
            scope,
        }))
    }

    fn resolve(&'a self, path: impl IntoIterator<Item = &'a str>) -> Option<QualName<'a>> {
        let mut path = path.into_iter();
        let base = path.next().expect("Invalid fully-qualified path");

        let (package, ScopeRef { mut scope, .. }) = self.resolve_one(base)?;

        Some(QualName {
            package: package.map(Into::into),
            path: std::iter::from_fn(|| {
                let Some(child) = scope.items().get(path.next()?) else { return Some(None) };
                scope = child;
                let Scope::Type { name, .. } = *child else { panic!("Invalid scope") };
                Some(Some(name.into()))
            })
            .collect::<Option<_>>()?,
        })
    }
}

type ScopeItems<'a> = HashMap<&'a str, Scope<'a>>;

#[derive(Debug)]
enum Scope<'a> {
    Package {
        name: Option<&'a str>,
        items: ScopeItems<'a>,
    },
    Type {
        name: &'a str,
        nested: ScopeItems<'a>,
    },
}

impl<'a> Scope<'a> {
    fn build_package(fildes: &'a prost_types::FileDescriptorProto) -> Self {
        Self::Package {
            name: fildes.package.as_deref(),
            items: Scope::build_items(&fildes.message_type, &fildes.enum_type),
        }
    }

    fn build_items(
        msgs: impl IntoIterator<Item = &'a prost_types::DescriptorProto>,
        enums: impl IntoIterator<Item = &'a prost_types::EnumDescriptorProto>,
    ) -> ScopeItems<'a> {
        msgs.into_iter()
            .map(|m| (m.name.as_deref().unwrap(), Self::build_msg(m)))
            .chain(
                enums
                    .into_iter()
                    .map(|e| (e.name.as_deref().unwrap(), Self::build_enum(e))),
            )
            .collect()
    }

    fn build_msg(msg: &'a prost_types::DescriptorProto) -> Self {
        Self::Type {
            name: msg.name.as_deref().unwrap(),
            nested: Self::build_items(&msg.nested_type, &msg.enum_type),
        }
    }

    fn build_enum(num: &'a prost_types::EnumDescriptorProto) -> Self {
        Self::Type {
            name: num.name.as_deref().unwrap(),
            nested: Self::build_items([], []),
        }
    }

    fn items(&self) -> &ScopeItems<'a> {
        match self {
            Self::Package { items, .. } | Self::Type { nested: items, .. } => items,
        }
    }
}

#[derive(Debug, Clone)]
struct ScopeRef<'a> {
    global: &'a GlobalScope<'a>,
    parent: Option<Rc<ScopeRef<'a>>>,
    scope: &'a Scope<'a>,
}

impl<'a> ScopeRef<'a> {
    #[inline]
    fn parent(&self) -> Option<&ScopeRef<'a>> { self.parent.as_deref() }

    fn ty<Q: Eq + Hash + ?Sized>(self, name: &Q) -> Option<ScopeRef<'a>>
    where &'a str: Borrow<Q> {
        self.scope.items().get(name).map(|scope| ScopeRef {
            global: self.global,
            parent: Some(self.into()),
            scope,
        })
    }

    fn qualify<'b, Q: Eq + Hash + ?Sized + 'b>(
        &self,
        path: impl IntoIterator<Item = &'b Q>,
    ) -> Option<QualName<'a>>
    where
        &'a str: Borrow<Q>,
    {
        let mut curr = Some(self);
        let mut package = None;
        let up = std::iter::from_fn(|| {
            let me = curr?;
            curr = me.parent.as_deref();
            match *me.scope {
                Scope::Package { name, .. } => {
                    package = name;
                    assert!(curr.is_none());
                    None
                },
                Scope::Type { name, .. } => Some(name),
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|s| Some(s.into()));

        let mut path = path.into_iter();
        let mut curr = self.scope;
        let down = std::iter::from_fn(|| {
            let Some(child) = curr.items().get(path.next()?) else { return Some(None) };
            curr = child;
            let Scope::Type { name, .. } = *child else { panic!("Invalid scope") };
            Some(Some(name.into()))
        });

        Some(QualName {
            package: package.map(Into::into),
            path: up.chain(down).collect::<Option<_>>()?,
        })
    }

    fn search_one(&self, name: &'a str) -> Option<Cow<'_, ScopeRef<'a>>> {
        if self.scope.items().contains_key(name) {
            Some(Cow::Borrowed(self))
        } else if let Some(ref parent) = self.parent {
            parent.search_one(name)
        } else {
            let Scope::Package { name: my_name, .. } = self.scope else {
                panic!("Invalid scope reference");
            };

            if my_name.map_or(false, |n| n == name) {
                Some(Cow::Borrowed(self))
            } else {
                self.global.resolve_one(name).map(|(_, s)| Cow::Owned(s))
            }
        }
    }

    #[inline]
    fn search(&self, path: impl IntoIterator<Item = &'a str>) -> Option<QualName<'a>> {
        let mut path = path.into_iter();
        let base = path.next().expect("Invalid type name");
        let owner = self.search_one(base);
        owner?.qualify(path)
    }
}

struct Visitor<'a>(&'a mut Schema);

impl<'a> Visitor<'a> {
    fn fildes_set(&mut self, desc: &FileDescriptorSet) {
        let scope = GlobalScope::build(desc);

        let FileDescriptorSet { file } = desc;

        file.iter().for_each(|f| self.fildes(&scope, f));
    }

    #[inline]
    fn descend(
        &mut self,
        scope: &ScopeRef<'_>,
        msgs: &[prost_types::DescriptorProto],
        enums: &[prost_types::EnumDescriptorProto],
    ) {
        for m in msgs {
            self.desc(&scope.clone().ty(m.name.as_deref().unwrap()).unwrap(), m);
        }

        for e in enums {
            self.enum_desc(&scope.clone().ty(e.name.as_deref().unwrap()).unwrap(), e);
        }
    }

    fn fildes(&mut self, scope: &GlobalScope<'_>, desc: &prost_types::FileDescriptorProto) {
        let prost_types::FileDescriptorProto {
            name: _,
            package,
            dependency,
            public_dependency,
            weak_dependency,
            message_type,
            enum_type,
            service,
            extension,
            options,
            source_code_info,
            syntax,
        } = desc;

        assert!(dependency.is_empty());
        assert!(public_dependency.is_empty());
        assert!(weak_dependency.is_empty());
        assert!(service.is_empty());
        assert!(extension.is_empty());
        assert!(options.is_none());
        assert!(source_code_info.is_none());
        assert_eq!(syntax.as_deref(), Some("proto3"));

        let scope = scope.package(&package.as_deref()).unwrap();

        self.descend(&scope, message_type, enum_type);
    }

    fn desc(&mut self, scope: &ScopeRef<'_>, desc: &prost_types::DescriptorProto) {
        let prost_types::DescriptorProto {
            name,
            field,
            extension,
            nested_type,
            enum_type,
            extension_range,
            oneof_decl,
            options,
            reserved_range,
            reserved_name,
        } = desc;

        let name = name.as_ref().unwrap();
        assert!(extension.is_empty());
        assert!(extension_range.is_empty());

        let qual_name = scope
            .parent()
            .and_then(|p| p.qualify([&**name]))
            .expect("Invalid message name");

        let (deprecated, is_for_map) = if let Some(opts) = options {
            let prost_types::MessageOptions {
                message_set_wire_format,
                no_standard_descriptor_accessor,
                deprecated,
                map_entry,
                uninterpreted_option,
            } = opts;

            assert!(message_set_wire_format.is_none());
            assert!(no_standard_descriptor_accessor.is_none());
            assert!(uninterpreted_option.is_empty());

            (deprecated.unwrap_or(false), map_entry.unwrap_or(false))
        } else {
            (false, false)
        };

        let mut numbers = HashMap::new();

        for field in field {
            Self::field(&mut numbers, scope, field);
        }

        for oneof in oneof_decl {
            let prost_types::OneofDescriptorProto { name: _, options } = oneof;

            assert!(options.is_none());
        }

        let reserved = if deprecated {
            ReservedMap::Deprecated
        } else {
            ReservedMap::build_reserved(reserved_range.iter().map(
                |prost_types::descriptor_proto::ReservedRange { start, end }| {
                    start.unwrap().into()..end.unwrap().into()
                },
            ))
        };

        let reserved_names: HashSet<_> = reserved_name.iter().cloned().collect();
        assert!(reserved_names.len() == reserved_name.len());

        assert!(
            self.0
                .types
                .insert(
                    qual_name.into_owned(),
                    Type::Message(Record::new(numbers, reserved, reserved_names, is_for_map))
                )
                .is_none()
        );

        self.descend(scope, nested_type, enum_type);
    }

    #[inline]
    fn field(
        numbers: &mut HashMap<i32, Field>,
        scope: &ScopeRef<'_>,
        field: &prost_types::FieldDescriptorProto,
    ) {
        let prost_types::FieldDescriptorProto {
            name,
            number,
            label,
            r#type,
            type_name,
            extendee,
            default_value: _,
            oneof_index,
            json_name: _,
            options,
            proto3_optional,
        } = field;

        let name = name.as_ref().unwrap();
        let number = number.unwrap();
        let label = label
            .and_then(prost_types::field_descriptor_proto::Label::from_i32)
            .unwrap();
        let ty = r#type.and_then(prost_types::field_descriptor_proto::Type::from_i32);
        let type_name = type_name.as_ref();
        assert!(extendee.is_none());

        let packed = if let Some(opts) = options {
            let prost_types::FieldOptions {
                ctype,
                packed,
                jstype,
                lazy,
                deprecated,
                weak,
                uninterpreted_option,
            } = opts;

            assert!(ctype.is_none());
            assert!(jstype.is_none());
            assert!(lazy.is_none());
            assert!(deprecated.is_none());
            assert!(weak.is_none());
            assert!(uninterpreted_option.is_empty());

            *packed
        } else {
            None
        };

        let field = Field {
            name: name.into(),
            ty: if let Some(ty) = ty.and_then(PrimitiveType::build) {
                assert!(type_name.is_none());
                FieldType::Primitive(ty)
            } else {
                let type_name = type_name.unwrap();

                let qual = if let Some(type_name) = type_name.strip_prefix('.') {
                    scope
                        .global
                        .resolve(type_name.split('.'))
                        .expect("Couldn't resolve fully-qualified type name")
                        .to_owned()
                } else {
                    scope
                        .search(type_name.split('.'))
                        .expect("Couldn't find valid scope for name")
                        .to_owned()
                };

                FieldType::Named(qual)
            },
            kind: FieldKind::build(label, packed, *proto3_optional),
            oneof: *oneof_index,
        };

        assert!(numbers.insert(number, field).is_none());
    }

    fn enum_desc(&mut self, scope: &ScopeRef<'_>, desc: &prost_types::EnumDescriptorProto) {
        let prost_types::EnumDescriptorProto {
            name,
            value,
            options,
            reserved_range,
            reserved_name,
        } = desc;

        let name = name.as_ref().unwrap();
        let qual_name = scope
            .parent()
            .and_then(|p| p.qualify([&**name]))
            .expect("Invalid message name");

        let mut numbers: HashMap<i32, Variant> = HashMap::new();

        let (aliasing, deprecated) = if let Some(opts) = options {
            let prost_types::EnumOptions {
                allow_alias,
                deprecated,
                uninterpreted_option,
            } = opts;

            assert!(uninterpreted_option.is_empty());

            (allow_alias.unwrap_or(false), deprecated.unwrap_or(false))
        } else {
            (false, false)
        };

        for value in value {
            let prost_types::EnumValueDescriptorProto {
                name,
                number,
                options,
            } = value;

            let name = name.as_ref().unwrap();
            let number = number.unwrap();
            assert!(options.is_none());

            if aliasing {
                assert!(numbers.entry(number).or_default().0.insert(name.into()));
            } else {
                assert!(
                    numbers
                        .insert(number, Variant([name.into()].into_iter().collect()))
                        .is_none()
                );
            }
        }

        let reserved = if deprecated {
            ReservedMap::Deprecated
        } else {
            ReservedMap::build_reserved(reserved_range.iter().map(
                |prost_types::enum_descriptor_proto::EnumReservedRange { start, end }| {
                    start.unwrap().into()..end.and_then(|i| i64::from(i).checked_add(1)).unwrap()
                },
            ))
        };

        let reserved_names: HashSet<_> = reserved_name.iter().cloned().collect();
        assert!(reserved_names.len() == reserved_name.len());

        assert!(
            self.0
                .types
                .insert(
                    qual_name.into_owned(),
                    Type::Enum(Record::new(numbers, reserved, reserved_names, false))
                )
                .is_none()
        );
    }
}

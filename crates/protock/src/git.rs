use std::path::Path;

use git2::{Commit, DiffOptions, Repository};

type Result<T> = std::result::Result<T, git2::Error>;

#[inline]
pub fn open() -> Result<Repository> { Repository::open_from_env() }

pub fn diff_opts(file: impl AsRef<Path>) -> DiffOptions {
    let mut diffopt = DiffOptions::new();
    diffopt.pathspec(file.as_ref());
    diffopt
}

pub fn log(
    repo: &'_ Repository,
    mut diffopt: DiffOptions,
) -> Result<impl Iterator<Item = Result<Commit>> + '_> {
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

pub fn commit_id(commit: &Commit) -> Result<git2::Buf> { commit.as_object().short_id() }

pub fn commit_file<'a>(
    repo: &'a Repository,
    commit: &'_ Commit,
    file: impl AsRef<Path>,
) -> Result<Option<git2::Blob<'a>>> {
    let tree = commit.tree()?;

    let entry = match tree.get_path(file.as_ref()) {
        Ok(e) => Some(e),
        Err(e) if e.code() == git2::ErrorCode::NotFound => None,
        Err(e) => return Err(e),
    };

    entry
        .map(|e| e.to_object(&repo).and_then(|o| o.peel_to_blob()))
        .transpose()
}

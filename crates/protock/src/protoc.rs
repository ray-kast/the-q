use std::{io::prelude::*, path::Path};

use anyhow::{Context, Result};
use prost::Message;
use prost_types::FileDescriptorSet;

pub fn get_descriptor_set<I: IntoIterator>(files: I) -> Result<FileDescriptorSet>
where I::Item: AsRef<Path> {
    let mut tmp = tempfile::NamedTempFile::new().context("Error creating descriptor tempfile")?;

    let mut cmd = std::process::Command::new("protoc");
    cmd.arg(format!("--descriptor_set_out={}", tmp.path().display()));

    files.into_iter().try_fold(&mut cmd, |c, f| {
        let f = f.as_ref();
        let dir = f
            .parent()
            .with_context(|| format!("Couldn't find parent dir for {f:?}"))?;
        Result::<_>::Ok(c.args([format!("-I{}", dir.display()).into(), f.to_owned()]))
    })?;

    let out = cmd.output().context("Error running protoc")?;
    for line in String::from_utf8_lossy(&out.stderr).lines() {
        if line.trim().is_empty() {
            continue;
        }

        tracing::warn!("{line}");
    }

    if !out.status.success() {
        anyhow::bail!(
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

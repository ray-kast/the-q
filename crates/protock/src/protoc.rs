use std::io::prelude::*;

use anyhow::{Context, Result};
use prost::Message;
use prost_types::FileDescriptorSet;

pub fn get_descriptor_set<I: IntoIterator>(files: I) -> Result<FileDescriptorSet>
where I::Item: AsRef<std::ffi::OsStr> {
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

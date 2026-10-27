//! Per-sample Engine DJ encryption with native MP4 editing and atomic files.
mod container;
mod crypto;

use anyhow::{Context, Result, ensure};
pub use container::{Converted, Operation, Stats, transform};
pub use crypto::{decrypt_sample, encrypt_sample};
use std::{
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// Conversion stopped before publishing the destination.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("conversion cancelled")
    }
}

impl std::error::Error for Cancelled {}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled.into());
    }
    Ok(())
}

/// Cancellation before publication leaves the destination unchanged.
pub fn transform_file(
    input: &Path,
    output: &Path,
    operation: Operation,
    cancel: &AtomicBool,
) -> Result<Stats> {
    check_cancel(cancel)?;
    if output.exists() {
        ensure!(
            !same_file::is_same_file(input, output)?,
            "input and output refer to the same file"
        );
    }
    let data = std::fs::read(input).context("reading input file")?;
    let result = transform(&data, operation, cancel)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).context("creating output file")?;
    temporary
        .write_all(&result.bytes)
        .context("writing output file")?;
    temporary
        .as_file()
        .sync_all()
        .context("syncing output file")?;
    check_cancel(cancel)?;
    temporary
        .persist(output)
        .map_err(|error| error.error)
        .context("publishing output file")?;
    Ok(result.stats)
}

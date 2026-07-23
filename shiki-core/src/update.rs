//! Self-update against GitHub Releases. Two explicit phases — `check_latest`
//! (a cheap API call, safe to run on every launch) and `install_latest` (the
//! real download + integrity check + binary replace) — so a caller can show
//! "update available" and only actually touch disk once the user confirms.
//!
//! Archive layout must match `.github/workflows/release.yml`'s packaging:
//! `shiki-v{version}-{target}/shiki` (or `shiki.exe` on Windows), which is
//! exactly what `BIN_PATH_TEMPLATE` below extracts.

use crate::{Error, Result};
use self_update::backends::github::Update;

const REPO_OWNER: &str = "sazardev";
const REPO_NAME: &str = "shiki";
const BIN_NAME: &str = "shiki";
const BIN_PATH_TEMPLATE: &str = "shiki-v{{ version }}-{{ target }}/{{ bin }}";

fn configured(current_version: &str) -> Result<self_update::backends::github::UpdateBuilder> {
    let mut builder = Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .target(self_update::get_target())
        .bin_path_in_archive(BIN_PATH_TEMPLATE)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        // GitHub computes and serves a sha256 digest per release asset; this
        // rejects the install if the download doesn't match it — verified
        // against the real repo, not just assumed to be present.
        .verify_release_digest(true)
        .current_version(current_version);
    Ok(builder)
}

/// Checks GitHub Releases for a version newer than `current_version`, without
/// downloading anything. `Ok(None)` means already up to date.
pub fn check_latest(current_version: &str) -> Result<Option<String>> {
    let updater = configured(current_version)?
        .build()
        .map_err(|e| Error::Update(e.to_string()))?;
    let newer = updater
        .is_update_available()
        .map_err(|e| Error::Update(e.to_string()))?;
    Ok(newer.map(|release| release.version().to_string()))
}

/// Downloads, verifies, and installs the latest release in place of the
/// currently running binary. Returns the installed version on success.
/// Does *not* relaunch the process — the caller decides whether/how to.
pub fn install_latest(current_version: &str) -> Result<String> {
    let updater = configured(current_version)?
        .build()
        .map_err(|e| Error::Update(e.to_string()))?;
    let status = updater.update().map_err(|e| Error::Update(e.to_string()))?;
    Ok(status.version().to_string())
}

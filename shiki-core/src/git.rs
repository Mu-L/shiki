use std::cell::Cell;
use std::path::Path;

use git2::{Cred, CredentialType, IndexAddOption, RemoteCallbacks, Repository, Signature};

use crate::{Error, Result};

/// Builds a credentials callback that actually works for more than SSH.
///
/// The previous version unconditionally called `Cred::ssh_key_from_agent`,
/// which is meaningless for an `https://` remote — libgit2 would report that
/// as a generic "authentication required but no callback" failure, not a
/// clear "wrong credential type" error. This tries, in order: the SSH agent
/// (only when the server actually offered `SSH_KEY`), then the *system*
/// git credential helper (`Cred::credential_helper`) — which reuses whatever
/// the user's own `git`/`gh` already has stored (macOS Keychain, Windows
/// Credential Manager, libsecret, a cached PAT, …), so if plain `git clone`
/// works in their shell without prompting, this does too — then finally
/// anonymous access (works for public repos over HTTPS). A capped attempt
/// counter avoids looping forever if the server keeps rejecting every kind.
fn build_callbacks() -> RemoteCallbacks<'static> {
    let attempts = Cell::new(0u32);
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |url, username_from_url, allowed| {
        let tries = attempts.get();
        attempts.set(tries + 1);
        if tries >= 5 {
            return Err(git2::Error::from_str(
                "too many failed authentication attempts",
            ));
        }

        if allowed.contains(CredentialType::SSH_KEY) {
            if let Ok(cred) = Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")) {
                return Ok(cred);
            }
        }
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
            || allowed.contains(CredentialType::DEFAULT)
        {
            if let Ok(config) = git2::Config::open_default() {
                if let Ok(cred) = Cred::credential_helper(&config, url, username_from_url) {
                    return Ok(cred);
                }
            }
        }
        if allowed.contains(CredentialType::USERNAME) {
            if let Some(user) = username_from_url {
                return Cred::username(user);
            }
        }
        Cred::default()
    });
    callbacks
}

/// Sync status of a notebook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatus {
    pub is_repo: bool,
    pub dirty: bool,
    pub ahead: usize,
    pub behind: usize,
}

/// Initializes a git repo at `path` if one doesn't already exist.
pub fn init_repo(path: &Path) -> Result<Repository> {
    Ok(match Repository::open(path) {
        Ok(repo) => repo,
        Err(_) => Repository::init(path)?,
    })
}

/// Stages all changes and creates a commit. Returns `false` if there was nothing to commit.
pub fn commit_all(path: &Path, message: &str) -> Result<bool> {
    let repo = init_repo(path)?;
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    if let Ok(head) = repo.head() {
        if let Ok(parent_commit) = head.peel_to_commit() {
            if parent_commit.tree_id() == tree_id {
                return Ok(false); // nothing to commit
            }
        }
    }

    let signature = repo
        .signature()
        .unwrap_or_else(|_| Signature::now("shiki", "shiki@localhost").unwrap());

    let parents: Vec<_> = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .into_iter()
        .collect();
    let parent_refs: Vec<&_> = parents.iter().collect();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parent_refs,
    )?;
    Ok(true)
}

/// Pushes to `remote`/`branch`, authenticating via SSH agent (for `git@…`
/// remotes) or the system git credential store (for `https://…` remotes).
pub fn push(path: &Path, remote: &str, branch: &str) -> Result<()> {
    let repo = Repository::open(path)?;
    let mut remote = repo.find_remote(remote)?;
    let mut opts = git2::PushOptions::new();
    opts.remote_callbacks(build_callbacks());
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[&refspec], Some(&mut opts))?;
    Ok(())
}

/// Pull (fetch + fast-forward merge) from `remote`, preferring `branch`.
/// Returns the branch name actually pulled — it can differ from `branch`
/// (see the fallback below), so callers should report it rather than
/// assuming the configured name was used.
pub fn pull(path: &Path, remote: &str, branch: &str) -> Result<String> {
    let repo = Repository::open(path)?;
    let mut remote_ref = repo.find_remote(remote)?;
    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(build_callbacks());
    // Tags aren't needed for a note-taking pull, and following them adds
    // extra lines to FETCH_HEAD, making the FETCH_HEAD-corruption issue
    // below more likely to trigger.
    opts.download_tags(git2::AutotagOption::None);

    // Fetch every branch the remote has — via the standard
    // `+refs/heads/*:refs/remotes/{remote}/*` refspec `repo.remote()` set up
    // when the remote was created — rather than a single hardcoded branch
    // name. A repo whose default branch isn't `main` (older repos default to
    // `master`, or the owner just named it something else) would otherwise
    // fail outright since `refs/heads/{branch}` wouldn't exist upstream.
    remote_ref.fetch(&[] as &[&str], Some(&mut opts), None)?;

    let prefix = format!("refs/remotes/{remote}/");
    let mut available = Vec::new();
    for reference in repo.references_glob(&format!("{prefix}*"))?.flatten() {
        if let Some(name) = reference.name() {
            if let Some(b) = name.strip_prefix(&prefix) {
                available.push(b.to_string());
            }
        }
    }

    // Prefer the configured branch; if it's not there, fall back to the
    // remote's one branch (the common single-branch-with-a-different-name
    // case). Ambiguous (multiple branches, none matching) is an error rather
    // than a guess.
    let resolved_branch = if available.iter().any(|b| b == branch) {
        branch.to_string()
    } else if let [only] = available.as_slice() {
        only.clone()
    } else if available.is_empty() {
        return Err(Error::Git(git2::Error::from_str(
            "remote has no branches to pull (is it empty?)",
        )));
    } else {
        return Err(Error::Git(git2::Error::from_str(&format!(
            "branch '{branch}' not found on remote; available: {}",
            available.join(", ")
        ))));
    };

    // Reading the commit id back off the remote-tracking ref we just fetched
    // into, instead of FETCH_HEAD: FETCH_HEAD's on-disk format has extra
    // "branch '...' of '...'" annotation text after the commit id (and can
    // span multiple lines) — git2's loose-reference parser doesn't expect
    // that, so `repo.find_reference("FETCH_HEAD")` can fail with "corrupted
    // loose reference file: FETCH_HEAD" even when the fetch itself
    // succeeded. A plain ref (just a commit id, no annotation) doesn't have
    // this problem.
    let tracking_ref = format!("{prefix}{resolved_branch}");
    let fetched_oid = repo.refname_to_id(&tracking_ref)?;
    let fetch_commit = repo.find_annotated_commit(fetched_oid)?;
    let refname = format!("refs/heads/{resolved_branch}");

    match repo.find_reference(&refname) {
        // Local branch exists — only fast-forward, never discard local commits.
        Ok(mut reference) => {
            let analysis = repo.merge_analysis(&[&fetch_commit])?;
            if analysis.0.is_fast_forward() {
                reference.set_target(fetch_commit.id(), "shiki: fast-forward")?;
                repo.set_head(&refname)?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            }
        }
        // No local branch yet — this is the first pull into a brand-new
        // notebook (e.g. importing an existing repo as its remote), so there's
        // nothing to fast-forward against. Point the branch straight at what
        // was fetched, the same initial checkout `git clone` would do.
        Err(_) => {
            repo.reference(&refname, fetch_commit.id(), true, "shiki: initial pull")?;
            repo.set_head(&refname)?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        }
    }
    Ok(resolved_branch)
}

/// Sets (creating or replacing) the notebook's `origin` remote. `url` can be
/// a normal git URL (`https://…`, `git@…`) or a local path/`file://` URL —
/// git treats a local path remote the same as any other for fetch/pull.
pub fn set_remote(path: &Path, url: &str) -> Result<()> {
    let repo = init_repo(path)?;
    if repo.find_remote("origin").is_ok() {
        repo.remote_set_url("origin", url)?;
    } else {
        repo.remote("origin", url)?;
    }
    Ok(())
}

/// The notebook's configured `origin` URL, if any.
pub fn remote_url(path: &Path) -> Option<String> {
    let repo = Repository::open(path).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    remote.url().map(String::from)
}

/// Quick status: whether the repo exists and has uncommitted changes.
pub fn status(path: &Path) -> GitStatus {
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(_) => {
            return GitStatus {
                is_repo: false,
                dirty: false,
                ahead: 0,
                behind: 0,
            }
        }
    };
    let dirty = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);
    GitStatus {
        is_repo: true,
        dirty,
        ahead: 0,
        behind: 0,
    }
}

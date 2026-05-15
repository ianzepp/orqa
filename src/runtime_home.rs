use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::symlink;

use crate::{model::FinRef, model::Orqa};

pub(crate) fn ensure_fin_runtime_homes(orqa: &Orqa, fin: &FinRef) -> Result<(), String> {
    // Phase 05-4: Use effective fin home (supports both new pod roots and legacy)
    let fin_home = orqa.effective_fin_home(fin);
    for runtime_dir in [".codex", ".hermes", ".pi/agent", ".pi/sessions", ".grok"] {
        let path = fin_home.join(runtime_dir);
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "failed to create fin runtime directory {}: {error}",
                path.display()
            )
        })?;
    }
    let _ = link_codex_auth(&fin_home.join(".codex/auth.json"));
    let _ = link_grok_auth(&fin_home.join(".grok/auth.json"));

    Ok(())
}

fn link_codex_auth(destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Ok(());
    }

    let Some(source) = user_codex_auth_path() else {
        return Ok(());
    };
    if !source.exists() || same_path(&source, destination) {
        return Ok(());
    }

    #[cfg(unix)]
    {
        symlink(&source, destination).map_err(|error| {
            format!(
                "failed to link Codex auth {} -> {}: {error}",
                destination.display(),
                source.display()
            )
        })
    }

    #[cfg(not(unix))]
    {
        fs::copy(&source, destination).map(|_| ()).map_err(|error| {
            format!(
                "failed to copy Codex auth {} -> {}: {error}",
                source.display(),
                destination.display()
            )
        })
    }
}

fn user_codex_auth_path() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex/auth.json"))
}

fn link_grok_auth(destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Ok(());
    }

    let Some(source) = user_grok_auth_path() else {
        return Ok(());
    };
    if !source.exists() || same_path(&source, destination) {
        return Ok(());
    }

    #[cfg(unix)]
    {
        symlink(&source, destination).map_err(|error| {
            format!(
                "failed to link Grok auth {} -> {}: {error}",
                destination.display(),
                source.display()
            )
        })
    }

    #[cfg(not(unix))]
    {
        fs::copy(&source, destination).map(|_| ()).map_err(|error| {
            format!(
                "failed to copy Grok auth {} -> {}: {error}",
                source.display(),
                destination.display()
            )
        })
    }
}

fn user_grok_auth_path() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".grok/auth.json"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

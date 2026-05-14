pub(crate) fn ensure_maildir(mail_home: &Path) -> Result<(), String> {
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(mail_home.join(dir)).map_err(|error| {
            format!("failed to create maildir {}: {error}", mail_home.display())
        })?;
    }

    Ok(())
}

pub(crate) fn deliver_mail(mail_home: &Path, message: &str) -> Result<PathBuf, String> {
    let name = unique_mail_name()?;
    let tmp_path = mail_home.join("tmp").join(&name);
    let new_path = mail_home.join("new").join(&name);

    fs::write(&tmp_path, message)
        .map_err(|error| format!("failed to write mail {}: {error}", tmp_path.display()))?;
    fs::rename(&tmp_path, &new_path).map_err(|error| {
        format!(
            "failed to move mail into inbox {}: {error}",
            new_path.display()
        )
    })?;

    Ok(new_path)
}

pub(crate) fn unread_count(mail_home: &Path) -> Result<usize, String> {
    Ok(sorted_files(&mail_home.join("new"))?.len())
}

pub(crate) fn sorted_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
        if entry.path().is_file() {
            paths.push(entry.path());
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn resolve_fin(pod: Option<&str>, fin: Option<&str>) -> Result<FinRef, String> {
    let pod = match pod {
        Some(pod) => pod.to_string(),
        None => env::var("ORQA_POD")
            .map_err(|_| "missing pod; use --pod or run with ORQA_POD set".to_string())?,
    };
    let fin = match fin {
        Some(fin) => fin.to_string(),
        None => env::var("ORQA_FIN")
            .map_err(|_| "missing fin; use --fin or run with ORQA_FIN set".to_string())?,
    };

    FinRef::new(&pod, &fin)
}

pub(crate) fn resolve_message_path(mail_home: &Path, message: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(message);
    if path.exists() {
        return Ok(path);
    }

    for state in ["new", "cur"] {
        let candidate = mail_home.join(state).join(message);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "message {message:?} not found in {}",
        mail_home.display()
    ))
}

pub(crate) fn message_id(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| format!("message path has no filename: {}", path.display()))
}

pub(crate) fn message_title(path: &Path, kind: ItemKind) -> Result<String, String> {
    let message = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    for line in message.lines() {
        if let Some(title) = line.strip_prefix(kind.title_header()) {
            return Ok(title.to_string());
        }
    }

    Ok("(no title)".to_string())
}

pub(crate) fn mail_state(mail_home: &Path, path: &Path) -> Result<&'static str, String> {
    if path.starts_with(mail_home.join("new")) {
        Ok("new")
    } else if path.starts_with(mail_home.join("cur")) {
        Ok("cur")
    } else {
        Err(format!(
            "message {} is not inside {}",
            path.display(),
            mail_home.display()
        ))
    }
}

pub(crate) fn unique_mail_name() -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    let counter = MAIL_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(format!(
        "{}.{}.{}.orqa",
        now.as_micros(),
        std::process::id(),
        counter
    ))
}

pub(crate) fn read_stdin() -> Result<String, String> {
    let mut body = String::new();
    io::stdin()
        .read_to_string(&mut body)
        .map_err(|error| format!("failed to read stdin: {error}"))?;
    Ok(body)
}

pub(crate) fn resolve_sender(from: Option<&str>) -> Result<MailAddress, String> {
    match from {
        Some(from) => {
            let pod = env::var("ORQA_POD").ok();
            resolve_address(from, pod.as_deref())
        }
        None => {
            let pod = env::var("ORQA_POD").map_err(|_| {
                "missing sender; use --from fin@pod.orqa or run with ORQA_POD and ORQA_FIN set"
                    .to_string()
            })?;
            let fin = env::var("ORQA_FIN").map_err(|_| {
                "missing sender; use --from fin@pod.orqa or run with ORQA_POD and ORQA_FIN set"
                    .to_string()
            })?;

            resolve_address(&fin, Some(&pod))
        }
    }
}

pub(crate) fn resolve_address(
    address: &str,
    pod_hint: Option<&str>,
) -> Result<MailAddress, String> {
    if address.contains('@') {
        return MailAddress::parse(address);
    }

    let pod = match pod_hint {
        Some(pod) => pod.to_string(),
        None => env::var("ORQA_POD").map_err(|_| {
            format!(
                "bare address {address:?} needs ORQA_POD; use fin@pod.orqa or run with ORQA_POD set"
            )
        })?,
    };

    validate_slug(address)?;
    validate_slug(&pod)?;

    Ok(MailAddress {
        fin: address.to_string(),
        pod,
    })
}

pub(crate) fn write_if_missing(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn write_sleep_marker(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("sleep marker path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create sleep marker directory {}: {error}",
            parent.display()
        )
    })?;
    fs::write(path, "sleeping=true\n")
        .map_err(|error| format!("failed to write sleep marker {}: {error}", path.display()))
}

pub(crate) fn remove_sleep_marker(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            format!("failed to remove sleep marker {}: {error}", path.display())
        })?;
    }

    Ok(())
}
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    mailbox::ItemKind,
    model::{FinRef, MailAddress, validate_slug},
};

static MAIL_COUNTER: AtomicUsize = AtomicUsize::new(0);

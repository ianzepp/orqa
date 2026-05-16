use std::{collections::BTreeMap, env, fs, path::PathBuf};

use toml::Table;

impl Orqa {
    pub(crate) fn new(home: Option<PathBuf>) -> Self {
        Self {
            home: home
                .or_else(|| env::var_os("ORQA_HOME").map(PathBuf::from))
                .unwrap_or_else(default_home),
        }
    }

    /// Returns true if the pod data home contains a `pod.toml` file.
    pub(crate) fn pod_exists(&self, pod: &PodRef) -> bool {
        self.pod_data_home(pod)
            .is_ok_and(|home| home.join("pod.toml").exists())
    }

    /// Returns true if the fin data home contains a `fin.toml` file.
    pub(crate) fn fin_exists(&self, fin: &FinRef) -> bool {
        self.fin_data_home(fin)
            .is_ok_and(|home| home.join("fin.toml").exists())
    }

    /// Returns Ok if the pod exists (has `pod.toml`), otherwise a friendly error
    /// suggesting the exact `orqa pod create` command.
    pub(crate) fn ensure_pod_exists(&self, pod: &PodRef) -> Result<(), String> {
        if self.pod_exists(pod) {
            Ok(())
        } else {
            Err(format!(
                "pod '{}' does not exist (run 'orqa pod create {}' to create it)",
                pod.slug, pod.slug
            ))
        }
    }

    /// Returns Ok if the fin exists (has `fin.toml`), otherwise a friendly error
    /// suggesting the exact `orqa fin create` command.
    pub(crate) fn ensure_fin_exists(&self, fin: &FinRef) -> Result<(), String> {
        if self.fin_exists(fin) {
            Ok(())
        } else {
            Err(format!(
                "fin '{}' does not exist (run 'orqa fin create {}' to create it)",
                fin.label(),
                fin.label()
            ))
        }
    }
}

pub(crate) struct PodRef {
    pub(crate) slug: String,
}

impl PodRef {
    pub(crate) fn new(slug: &str) -> Result<Self, String> {
        validate_slug(slug)?;
        Ok(Self {
            slug: slug.to_string(),
        })
    }
}

pub(crate) struct FinRef {
    pub(crate) pod: String,
    pub(crate) fin: String,
}

impl FinRef {
    pub(crate) fn new(pod: &str, fin: &str) -> Result<Self, String> {
        validate_slug(pod)?;
        validate_slug(fin)?;
        Ok(Self {
            pod: pod.to_string(),
            fin: fin.to_string(),
        })
    }

    pub(crate) fn label(&self) -> String {
        format!("{}/{}", self.pod, self.fin)
    }
}

#[derive(Clone)]
pub(crate) struct MailAddress {
    pub(crate) fin: String,
    pub(crate) pod: String,
}

impl MailAddress {
    pub(crate) fn parse(address: &str) -> Result<Self, String> {
        let (fin, domain) = address
            .split_once('@')
            .ok_or_else(|| format!("invalid local address {address:?}; expected fin@pod.orqa"))?;
        let pod = domain
            .strip_suffix(".orqa")
            .ok_or_else(|| format!("invalid local address {address:?}; expected fin@pod.orqa"))?;

        validate_slug(fin)?;
        validate_slug(pod)?;

        Ok(Self {
            fin: fin.to_string(),
            pod: pod.to_string(),
        })
    }

    pub(crate) fn label(&self) -> String {
        format!("{}@{}.orqa", self.fin, self.pod)
    }
}

pub(crate) fn validate_slug(slug: &str) -> Result<(), String> {
    let valid = !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid slug {slug:?}; use lowercase letters, digits, and hyphens"
        ))
    }
}

pub(crate) fn default_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".orqa")
}

/// Represents a pod registered in the global `~/.orqa/config.toml`.
/// The `path` is the user's real pod root directory (e.g. `~/work/my-project`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PodRegistration {
    pub(crate) slug: String,
    pub(crate) path: PathBuf,
    pub(crate) enabled: bool,
}

pub(crate) fn load_registry(orqa: &Orqa) -> Result<BTreeMap<String, PodRegistration>, String> {
    let config_path = orqa.home.join("config.toml");
    if !config_path.exists() {
        return Ok(BTreeMap::new());
    }

    let contents = fs::read_to_string(&config_path).map_err(|e| {
        format!(
            "failed to read global config {}: {e}",
            config_path.display()
        )
    })?;

    let table: Table = contents.parse().map_err(|e| {
        format!(
            "failed to parse global config {}: {e}",
            config_path.display()
        )
    })?;

    let _registry_table = table.get("registry").and_then(|v| v.as_table());
    let pods_table = table.get("pods").and_then(|v| v.as_table());

    let mut regs = BTreeMap::new();

    if let Some(pods) = pods_table {
        for (slug, value) in pods {
            let pod_table = match value.as_table() {
                Some(t) => t,
                None => continue,
            };
            let path_str = match pod_table.get("path").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };
            let enabled = pod_table
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let expanded = if let Some(stripped) = path_str.strip_prefix("~/") {
                if let Some(home) = env::var_os("HOME") {
                    PathBuf::from(home).join(stripped)
                } else {
                    PathBuf::from(path_str)
                }
            } else {
                PathBuf::from(path_str)
            };

            // Make absolute if possible
            let final_path = if expanded.is_absolute() {
                expanded
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(expanded)
            };

            validate_slug(slug)?;

            regs.insert(
                slug.clone(),
                PodRegistration {
                    slug: slug.clone(),
                    path: final_path,
                    enabled,
                },
            );
        }
    }

    Ok(regs)
}

pub(crate) struct Orqa {
    pub(crate) home: PathBuf,
}

impl Orqa {
    /// Returns the real filesystem root directory for a registered pod.
    pub(crate) fn pod_root_for_slug(&self, slug: &str) -> Result<PathBuf, String> {
        if let Some((detected_slug, root)) = detect_pod_context() {
            if detected_slug == slug {
                return Ok(root);
            }
        }

        let regs = load_registry(self)?;
        match regs.get(slug) {
            Some(reg) if reg.enabled => Ok(reg.path.clone()),
            Some(_) => Err(format!("pod '{slug}' is registered but disabled")),
            None => Err(format!(
                "pod '{slug}' is not registered. Run 'orqa init {slug} --path <pod-root>' or cd into a pod root containing .orqa/pod.toml"
            )),
        }
    }

    pub(crate) fn pod_root(&self, pod: &PodRef) -> Result<PathBuf, String> {
        self.pod_root_for_slug(&pod.slug)
    }

    /// Returns the `.orqa` data directory inside the pod root.
    pub(crate) fn pod_data_home(&self, pod: &PodRef) -> Result<PathBuf, String> {
        Ok(self.pod_root(pod)?.join(".orqa"))
    }

    /// Returns the fin home directory under the pod's `.orqa/fins/<fin>`.
    pub(crate) fn fin_data_home(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self
            .pod_root_for_slug(&fin.pod)?
            .join(".orqa")
            .join("fins")
            .join(&fin.fin))
    }

    pub(crate) fn mail_home(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("mail"))
    }

    pub(crate) fn task_home(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("tasks"))
    }

    pub(crate) fn lock_path(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("run.lock"))
    }

    pub(crate) fn runs_home(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("runs"))
    }

    pub(crate) fn run_home(&self, fin: &FinRef, run: &str) -> Result<PathBuf, String> {
        Ok(self.runs_home(fin)?.join(run))
    }

    pub(crate) fn latest_run_path(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("latest-run"))
    }

    pub(crate) fn runs_ledger_path(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("runs.jsonl"))
    }

    pub(crate) fn pod_sleep_path(&self, pod: &PodRef) -> Result<PathBuf, String> {
        Ok(self.pod_data_home(pod)?.join("sleep.lock"))
    }

    pub(crate) fn fin_sleep_path(&self, fin: &FinRef) -> Result<PathBuf, String> {
        Ok(self.fin_data_home(fin)?.join("sleep.lock"))
    }

    pub(crate) fn pod_hooks_home(&self, pod: &PodRef) -> Result<PathBuf, String> {
        Ok(self.pod_data_home(pod)?.join("hooks"))
    }

    pub(crate) fn pod_hook_phase_home(&self, pod: &PodRef, phase: &str) -> Result<PathBuf, String> {
        Ok(self.pod_hooks_home(pod)?.join(phase))
    }

    pub(crate) fn pod_hook_state_home(&self, pod: &PodRef, hook: &str) -> Result<PathBuf, String> {
        Ok(self.pod_hooks_home(pod)?.join("state").join(hook))
    }
}

/// Walks upward from the current working directory looking for a directory
/// that contains `.orqa/pod.toml`. Returns (slug, pod_root) if found.
/// The slug is currently derived from the directory name of the pod root.
pub(crate) fn detect_pod_context() -> Option<(String, PathBuf)> {
    let mut current = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    loop {
        let marker = current.join(".orqa").join("pod.toml");
        if marker.exists() {
            // Use the directory name as the slug for now
            if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
                // Basic validation that it would be a valid slug
                if validate_slug(name).is_ok() {
                    return Some((name.to_string(), current));
                }
            }
            // If the directory name is not a valid slug, we still found a pod
            // but can't use it cleanly — treat as not detected for safety.
            return None;
        }

        // Move to parent
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    None
}

/// Resolves the pod slug and root directory using the standard precedence:
/// 1. Explicit CLI argument (if provided)
/// 2. ORQA_POD environment variable
/// 3. Local filesystem detection (nearest .orqa/pod.toml)
///
/// Returns (slug, pod_root_path).
/// If nothing is found, returns an error with a helpful message.
pub(crate) fn resolve_pod_context(
    cli_pod: Option<String>,
    orqa: &Orqa,
) -> Result<(String, PathBuf), String> {
    // 1. Explicit CLI arg
    if let Some(slug) = cli_pod {
        if let Some((detected_slug, root)) = detect_pod_context() {
            if detected_slug == slug {
                return Ok((slug, root));
            }
        }
        let root = orqa.pod_root_for_slug(&slug)?;
        return Ok((slug, root));
    }

    // 2. ORQA_POD env
    if let Ok(slug) = env::var("ORQA_POD") {
        if let Some((detected_slug, root)) = detect_pod_context() {
            if detected_slug == slug {
                return Ok((slug, root));
            }
        }
        let root = orqa.pod_root_for_slug(&slug)?;
        return Ok((slug, root));
    }

    // 3. Filesystem detection
    if let Some((slug, root)) = detect_pod_context() {
        return Ok((slug, root));
    }

    Err(
        "missing pod: no pod specified and no pod detected in current directory tree. \
         Pass a pod slug, set ORQA_POD, or cd into a pod root that contains .orqa/pod.toml"
            .to_string(),
    )
}

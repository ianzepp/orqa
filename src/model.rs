use std::{env, path::PathBuf};

impl Orqa {
    pub(crate) fn new(home: Option<PathBuf>) -> Self {
        Self {
            home: home
                .or_else(|| env::var_os("ORQA_HOME").map(PathBuf::from))
                .unwrap_or_else(default_home),
        }
    }

    pub(crate) fn pod_home(&self, pod: &PodRef) -> PathBuf {
        self.home.join("pods").join(&pod.slug)
    }

    pub(crate) fn fin_home(&self, fin: &FinRef) -> PathBuf {
        self.home
            .join("pods")
            .join(&fin.pod)
            .join("fins")
            .join(&fin.fin)
    }

    pub(crate) fn mail_home(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("mail")
    }

    pub(crate) fn task_home(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("tasks")
    }

    pub(crate) fn lock_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("run.lock")
    }

    pub(crate) fn runs_home(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("runs")
    }

    pub(crate) fn run_home(&self, fin: &FinRef, run: &str) -> PathBuf {
        self.runs_home(fin).join(run)
    }

    pub(crate) fn runs_ledger_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("runs.jsonl")
    }

    pub(crate) fn latest_run_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("latest-run")
    }

    pub(crate) fn pod_sleep_path(&self, pod: &PodRef) -> PathBuf {
        self.pod_home(pod).join("sleep.lock")
    }

    pub(crate) fn fin_sleep_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("sleep.lock")
    }

    pub(crate) fn pod_hooks_home(&self, pod: &PodRef) -> PathBuf {
        self.pod_home(pod).join("hooks")
    }

    pub(crate) fn pod_hook_phase_home(&self, pod: &PodRef, phase: &str) -> PathBuf {
        self.pod_hooks_home(pod).join(phase)
    }

    pub(crate) fn pod_hook_state_home(&self, pod: &PodRef, hook: &str) -> PathBuf {
        self.pod_hooks_home(pod).join("state").join(hook)
    }

    /// Returns true if the pod home contains a `pod.toml` file.
    pub(crate) fn pod_exists(&self, pod: &PodRef) -> bool {
        self.pod_home(pod).join("pod.toml").exists()
    }

    /// Returns true if the fin home contains a `fin.toml` file.
    pub(crate) fn fin_exists(&self, fin: &FinRef) -> bool {
        self.fin_home(fin).join("fin.toml").exists()
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
pub(crate) struct Orqa {
    pub(crate) home: PathBuf,
}

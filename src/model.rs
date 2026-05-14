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

    pub(crate) fn pod_sleep_path(&self, pod: &PodRef) -> PathBuf {
        self.pod_home(pod).join("sleep.lock")
    }

    pub(crate) fn fin_sleep_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("sleep.lock")
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

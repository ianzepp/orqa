use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::{
    cli::{OpsIssueListArgs, OpsIssueReadArgs, OpsIssueResolutionArgs},
    mailbox::{
        deliver_mail, ensure_maildir, field_value, message_id, remove_sleep_marker, sorted_files,
        split_front_matter, upsert_field,
    },
    model::{FinRef, MailAddress, Orqa},
    status::print_json,
};

static ISSUE_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IssueSummary {
    pub(crate) state: String,
    pub(crate) id: String,
    pub(crate) pod: String,
    pub(crate) fin: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) severity: String,
    pub(crate) kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IssueList {
    pub(crate) issues: Vec<IssueSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IssueDetail {
    pub(crate) state: String,
    pub(crate) id: String,
    pub(crate) path: PathBuf,
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) body: String,
}

pub(crate) fn create_operator_issue(
    orqa: &Orqa,
    from: &MailAddress,
    subject: &str,
    body: &str,
) -> Result<PathBuf, String> {
    ensure_issue_dirs(orqa)?;
    let id = issue_id()?;
    let (mut fields, description) = split_front_matter(body);
    upsert_field(&mut fields, "id", &id);
    upsert_field(&mut fields, "from", &from.label());
    upsert_field(&mut fields, "to", &format!("operator@{}.orqa", from.pod));
    upsert_field(&mut fields, "pod", &from.pod);
    upsert_field(&mut fields, "fin", &from.fin);
    upsert_field(&mut fields, "title", subject);
    upsert_field(&mut fields, "status", "open");
    ensure_field(&mut fields, "severity", "needs-input");
    ensure_field(&mut fields, "kind", "other");
    ensure_field(&mut fields, "source", "operator-mail");
    ensure_field(&mut fields, "created_at", &now_epoch()?);

    deliver_issue(
        &orqa.issues_home(),
        &id,
        &render_issue(&fields, description),
    )
}

pub(crate) fn list_issues(orqa: &Orqa, args: OpsIssueListArgs) -> Result<(), String> {
    ensure_issue_dirs(orqa)?;
    let filters = IssueFilters::new(&args)?;
    let mut issues = collect_issues(orqa, args.all)?;
    issues.retain(|issue| filters.matches(issue));
    if args.json {
        return print_json(&IssueList { issues });
    }

    for issue in issues {
        println!(
            "{} {} pod={} fin={} severity={} status={} kind={} title={}",
            issue.state,
            issue.id,
            issue.pod,
            issue.fin,
            issue.severity,
            issue.status,
            issue.kind,
            quote_value(&issue.title)
        );
    }
    Ok(())
}

pub(crate) fn read_issue(orqa: &Orqa, args: OpsIssueReadArgs) -> Result<(), String> {
    let path = resolve_issue_path(orqa, &args.issue)?;
    if args.json {
        return print_json(&read_issue_detail(orqa, &path)?);
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read issue {}: {error}", path.display()))?;
    print!("{contents}");
    Ok(())
}

pub(crate) fn ack_issue(orqa: &Orqa, args: OpsIssueReadArgs) -> Result<(), String> {
    let path = resolve_issue_path(orqa, &args.issue)?;
    let updated = update_issue_fields(&path, &[("status", "acknowledged")])?;
    let id = issue_file_name(&path)?;
    let next = orqa.issues_home().join("cur").join(&id);
    move_issue_file(&updated, &next)?;
    if args.json {
        return print_json(&read_issue_detail(orqa, &next)?);
    }
    println!("{}", next.display());
    Ok(())
}

pub(crate) fn resolve_issue(
    orqa: &Orqa,
    args: OpsIssueResolutionArgs,
    terminal_status: &str,
) -> Result<(), String> {
    let path = resolve_issue_path(orqa, &args.issue)?;
    let detail = read_issue_detail(orqa, &path)?;
    let note = args
        .note
        .unwrap_or_else(|| format!("Issue {terminal_status}."));
    let time_key = if terminal_status == "resolved" {
        "resolved_at"
    } else {
        "dismissed_at"
    };
    let updated = update_issue_fields(
        &path,
        &[
            ("status", terminal_status),
            (time_key, &now_epoch()?),
            ("resolution", &note),
        ],
    )?;
    let id = issue_file_name(&path)?;
    let next = orqa.issues_home().join("closed").join(&id);
    move_issue_file(&updated, &next)?;
    mail_issue_resolution(orqa, &detail, terminal_status, &note)?;
    if args.wake {
        let fin = issue_fin(&detail)?;
        remove_sleep_marker(&orqa.fin_sleep_path(&fin))?;
        println!("wake {} reason=issue-{}", fin.label(), terminal_status);
    }
    println!("{}", next.display());
    Ok(())
}

pub(crate) fn issue_counts(orqa: &Orqa) -> Result<(usize, usize, usize), String> {
    ensure_issue_dirs(orqa)?;
    Ok((
        sorted_files(&orqa.issues_home().join("new"))?.len(),
        sorted_files(&orqa.issues_home().join("cur"))?.len(),
        sorted_files(&orqa.issues_home().join("closed"))?.len(),
    ))
}

fn ensure_issue_dirs(orqa: &Orqa) -> Result<(), String> {
    ensure_maildir(&orqa.issues_home())?;
    fs::create_dir_all(orqa.issues_home().join("closed")).map_err(|error| {
        format!(
            "failed to create issue closed directory {}: {error}",
            orqa.issues_home().join("closed").display()
        )
    })
}

fn deliver_issue(issue_home: &Path, id: &str, issue: &str) -> Result<PathBuf, String> {
    let tmp_path = issue_home.join("tmp").join(id);
    let new_path = issue_home.join("new").join(id);
    fs::write(&tmp_path, issue)
        .map_err(|error| format!("failed to write issue {}: {error}", tmp_path.display()))?;
    fs::rename(&tmp_path, &new_path).map_err(|error| {
        format!(
            "failed to move issue into inbox {}: {error}",
            new_path.display()
        )
    })?;
    Ok(new_path)
}

fn collect_issues(orqa: &Orqa, include_closed: bool) -> Result<Vec<IssueSummary>, String> {
    let mut issues = Vec::new();
    collect_issues_in_state(orqa, "new", &mut issues)?;
    collect_issues_in_state(orqa, "cur", &mut issues)?;
    if include_closed {
        collect_issues_in_state(orqa, "closed", &mut issues)?;
    }
    issues.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(issues)
}

struct IssueFilters {
    fields: Vec<(String, String)>,
}

impl IssueFilters {
    fn new(args: &OpsIssueListArgs) -> Result<Self, String> {
        let mut fields = Vec::new();

        if let Some(pod) = &args.pod {
            fields.push(("pod".to_string(), pod.to_string()));
        }
        if let Some(fin) = &args.fin {
            fields.push(("fin".to_string(), fin.to_string()));
        }
        if let Some(status) = &args.status {
            fields.push(("status".to_string(), status.to_string()));
        }
        if let Some(severity) = &args.severity {
            fields.push(("severity".to_string(), severity.to_string()));
        }
        if let Some(kind) = &args.kind {
            fields.push(("kind".to_string(), kind.to_string()));
        }
        for field in &args.fields {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| format!("invalid --field {field:?}; expected key=value"))?;
            if key.trim().is_empty() {
                return Err(format!("invalid --field {field:?}; key cannot be empty"));
            }
            fields.push((key.trim().to_string(), value.trim().to_string()));
        }

        Ok(Self { fields })
    }

    fn matches(&self, issue: &IssueSummary) -> bool {
        self.fields.iter().all(|(key, value)| {
            issue
                .field(key)
                .is_some_and(|issue_value| issue_value == value)
        })
    }
}

impl IssueSummary {
    fn field(&self, key: &str) -> Option<&str> {
        match key {
            "state" => Some(&self.state),
            "id" => Some(&self.id),
            "pod" => Some(&self.pod),
            "fin" => Some(&self.fin),
            "title" => Some(&self.title),
            "status" => Some(&self.status),
            "severity" => Some(&self.severity),
            "kind" => Some(&self.kind),
            _ => None,
        }
        .map(String::as_str)
    }
}

fn collect_issues_in_state(
    orqa: &Orqa,
    state: &str,
    issues: &mut Vec<IssueSummary>,
) -> Result<(), String> {
    for path in sorted_files(&orqa.issues_home().join(state))? {
        let detail = read_issue_detail(orqa, &path)?;
        issues.push(IssueSummary {
            state: state.to_string(),
            id: detail.id,
            pod: issue_field(&detail.fields, "pod"),
            fin: issue_field(&detail.fields, "fin"),
            title: issue_field(&detail.fields, "title"),
            status: issue_field(&detail.fields, "status"),
            severity: issue_field(&detail.fields, "severity"),
            kind: issue_field(&detail.fields, "kind"),
        });
    }
    Ok(())
}

fn read_issue_detail(orqa: &Orqa, path: &Path) -> Result<IssueDetail, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read issue {}: {error}", path.display()))?;
    let (fields, body) = split_front_matter(&contents);
    Ok(IssueDetail {
        state: issue_state(orqa, path)?,
        id: message_id(path)?,
        path: path.to_path_buf(),
        fields,
        body: body.to_string(),
    })
}

fn update_issue_fields(path: &Path, values: &[(&str, &str)]) -> Result<PathBuf, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read issue {}: {error}", path.display()))?;
    let (mut fields, body) = split_front_matter(&contents);
    for (key, value) in values {
        upsert_field(&mut fields, key, value);
    }
    fs::write(path, render_issue(&fields, body))
        .map_err(|error| format!("failed to write issue {}: {error}", path.display()))?;
    Ok(path.to_path_buf())
}

fn move_issue_file(from: &Path, to: &Path) -> Result<(), String> {
    if from == to {
        return Ok(());
    }
    fs::rename(from, to).map_err(|error| {
        format!(
            "failed to move issue {} -> {}: {error}",
            from.display(),
            to.display()
        )
    })
}

fn mail_issue_resolution(
    orqa: &Orqa,
    issue: &IssueDetail,
    status: &str,
    note: &str,
) -> Result<(), String> {
    let pod = issue_field(&issue.fields, "pod");
    let fin = issue_field(&issue.fields, "fin");
    let title = issue_field(&issue.fields, "title");
    let fin_ref = FinRef::new(&pod, &fin)?;
    let mail_home = orqa.mail_home(&fin_ref);
    ensure_maildir(&mail_home)?;
    let message = format!(
        "From: operator@{}.orqa\nTo: {}@{}.orqa\nSubject: Re: {}\n\nIssue {}.\n\n{}\n\nIssue: {}\n",
        pod, fin, pod, title, status, note, issue.id
    );
    deliver_mail(&mail_home, &message)?;
    Ok(())
}

fn issue_fin(issue: &IssueDetail) -> Result<FinRef, String> {
    FinRef::new(
        &issue_field(&issue.fields, "pod"),
        &issue_field(&issue.fields, "fin"),
    )
}

fn resolve_issue_path(orqa: &Orqa, issue: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(issue);
    if path.exists() {
        return Ok(path);
    }

    for state in ["new", "cur", "closed"] {
        let candidate = orqa.issues_home().join(state).join(issue);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "issue {issue:?} not found in {}",
        orqa.issues_home().display()
    ))
}

fn issue_state(orqa: &Orqa, path: &Path) -> Result<String, String> {
    for state in ["new", "cur", "closed"] {
        if path.starts_with(orqa.issues_home().join(state)) {
            return Ok(state.to_string());
        }
    }
    Err(format!(
        "issue {} is not inside {}",
        path.display(),
        orqa.issues_home().display()
    ))
}

fn issue_file_name(path: &Path) -> Result<String, String> {
    message_id(path)
}

fn issue_field(fields: &[(String, String)], key: &str) -> String {
    field_value(fields, key).unwrap_or_else(|| "-".to_string())
}

fn ensure_field(fields: &mut Vec<(String, String)>, key: &str, value: &str) {
    if field_value(fields, key).is_none() {
        fields.push((key.to_string(), value.to_string()));
    }
}

fn render_issue(fields: &[(String, String)], body: &str) -> String {
    let mut issue = String::from("---\n");
    for (key, value) in fields {
        issue.push_str(key);
        issue.push_str(": ");
        issue.push_str(&format_yaml_value(value));
        issue.push('\n');
    }
    issue.push_str("---\n\n");
    issue.push_str(body.trim());
    issue.push('\n');
    issue
}

fn issue_id() -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    let counter = ISSUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(format!(
        "{}.{}.{}.issue",
        now.as_micros(),
        std::process::id(),
        counter
    ))
}

fn now_epoch() -> Result<String, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_secs()
        .to_string())
}

fn quote_value(value: &str) -> String {
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'/' | b':' | b'[' | b']')
    }) {
        return value.to_string();
    }

    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn format_yaml_value(value: &str) -> String {
    if value == "[]" || (value.starts_with('[') && value.ends_with(']')) {
        return value.to_string();
    }
    if !needs_quoted_yaml_string(value) {
        return value.to_string();
    }

    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

fn needs_quoted_yaml_string(value: &str) -> bool {
    value.is_empty()
        || value.contains(": ")
        || value.contains(" #")
        || value.contains('\n')
        || matches!(
            value,
            "true" | "false" | "null" | "Null" | "NULL" | "~" | "yes" | "no" | "on" | "off"
        )
        || value.chars().next().is_some_and(|first| {
            matches!(first, '-' | '?' | ':' | '!' | '&' | '*' | '#' | '@' | '`')
        })
}

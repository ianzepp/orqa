pub(crate) struct TaskFilters {
    pub(crate) fields: Vec<(String, String)>,
}

impl TaskFilters {
    pub(crate) fn new(args: &TaskListArgs) -> Result<Self, String> {
        let mut fields = Vec::new();

        if let Some(status) = &args.status {
            fields.push(("status".to_string(), status.to_string()));
        }
        if let Some(priority) = &args.priority {
            fields.push(("priority".to_string(), priority.to_string()));
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

    pub(super) fn matches(&self, task: &TaskSummary) -> bool {
        self.fields.iter().all(|(key, value)| {
            task.field(key)
                .is_some_and(|task_value| task_value == value)
        })
    }
}

pub(crate) struct TaskSummary {
    state: String,
    id: String,
    fields: Vec<(String, String)>,
}

impl TaskSummary {
    fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(field_key, _)| field_key == key)
            .map(|(_, value)| value.as_str())
    }

    fn sort_value(&self, key: &str) -> String {
        match key {
            "state" => self.state.clone(),
            "id" => self.id.clone(),
            "priority" => priority_sort_value(self.field("priority").unwrap_or("")),
            key => self.field(key).unwrap_or("").to_string(),
        }
    }

    pub(super) fn format(&self) -> String {
        let mut line = format!("{} {}", self.state, self.id);
        for key in ["priority", "status", "kind", "title"] {
            if let Some(value) = self.field(key) {
                line.push(' ');
                line.push_str(key);
                line.push('=');
                line.push_str(&quote_value(value));
            }
        }
        line
    }
}

pub(crate) fn collect_tasks(
    task_home: &Path,
    include_done: bool,
) -> Result<Vec<TaskSummary>, String> {
    let mut tasks = Vec::new();

    collect_tasks_in_state(task_home, "new", &mut tasks)?;
    if include_done {
        collect_tasks_in_state(task_home, "cur", &mut tasks)?;
    }

    Ok(tasks)
}

pub(crate) fn collect_tasks_in_state(
    task_home: &Path,
    state: &str,
    tasks: &mut Vec<TaskSummary>,
) -> Result<(), String> {
    for path in sorted_files(&task_home.join(state))? {
        let body = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let (fields, _) = split_front_matter(&body);
        tasks.push(TaskSummary {
            state: state.to_string(),
            id: message_id(&path)?,
            fields,
        });
    }

    Ok(())
}

pub(crate) fn sort_tasks(tasks: &mut [TaskSummary], sort: Option<&str>, reverse: bool) {
    let sort = sort.unwrap_or("id");
    tasks.sort_by(|left, right| {
        left.sort_value(sort)
            .cmp(&right.sort_value(sort))
            .then_with(|| left.id.cmp(&right.id))
    });

    if reverse {
        tasks.reverse();
    }
}

pub(crate) fn priority_sort_value(priority: &str) -> String {
    let rank = match priority {
        "critical" | "urgent" => 0,
        "high" => 1,
        "normal" | "medium" => 2,
        "low" => 3,
        _ => 9,
    };

    format!("{rank}:{priority}")
}

pub(crate) fn quote_value(value: &str) -> String {
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'/' | b':' | b'[' | b']')
    }) {
        return value.to_string();
    }

    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
pub(crate) fn canonical_task_body(
    from: &MailAddress,
    to: &MailAddress,
    title_arg: Option<&str>,
    body: &str,
) -> String {
    let (mut fields, description) = split_front_matter(body);
    let title = title_arg
        .map(str::to_string)
        .or_else(|| field_value(&fields, "title"))
        .unwrap_or_else(|| "(untitled task)".to_string());

    upsert_field(&mut fields, "from", &from.label());
    upsert_field(&mut fields, "to", &to.label());
    upsert_field(&mut fields, "title", &title);
    ensure_field(&mut fields, "priority", "normal");
    ensure_field(&mut fields, "status", "open");
    ensure_field(&mut fields, "kind", "need");
    ensure_field(&mut fields, "depends_on", "[]");

    let mut task = String::from("---\n");
    for (key, value) in fields {
        task.push_str(&key);
        task.push_str(": ");
        task.push_str(&value);
        task.push('\n');
    }
    task.push_str("---\n\n");
    task.push_str(description.trim());
    task.push('\n');

    task
}

pub(crate) fn split_front_matter(body: &str) -> (Vec<(String, String)>, &str) {
    let Some(rest) = body.strip_prefix("---\n") else {
        return (Vec::new(), body);
    };
    let Some((front_matter, description)) = rest.split_once("\n---") else {
        return (Vec::new(), body);
    };

    let description = description.strip_prefix('\n').unwrap_or(description);
    (parse_front_matter(front_matter), description)
}

pub(crate) fn parse_front_matter(front_matter: &str) -> Vec<(String, String)> {
    front_matter
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), value.trim().to_string()))
        })
        .collect()
}

pub(crate) fn field_value(fields: &[(String, String)], key: &str) -> Option<String> {
    fields
        .iter()
        .find(|(existing_key, _)| existing_key == key)
        .map(|(_, value)| value.to_string())
}

pub(crate) fn ensure_field(fields: &mut Vec<(String, String)>, key: &str, value: &str) {
    if field_value(fields, key).is_none() {
        fields.push((key.to_string(), value.to_string()));
    }
}

pub(crate) fn upsert_field(fields: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some((_, existing_value)) = fields
        .iter_mut()
        .find(|(existing_key, _)| existing_key == key)
    {
        *existing_value = value.to_string();
    } else {
        fields.push((key.to_string(), value.to_string()));
    }
}
use std::{fs, path::Path};

use crate::{
    cli::TaskListArgs,
    mailbox::storage::{message_id, sorted_files},
    model::MailAddress,
};

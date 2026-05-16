use std::{collections::BTreeMap, ffi::OsString, fs, time::Duration};

use toml::{Table, Value};

use crate::model::{FinRef, Orqa, PodRef};

pub(crate) const DEFAULT_CHARTER: &str = "No pod charter has been set yet.";
pub(crate) const DEFAULT_ROLE: &str = "No fin role has been set yet.";

pub(crate) fn pod_agents_template(pod: &PodRef, charter: &str) -> String {
    render_agents_template(
        include_str!("../templates/pod-agents.md"),
        &pod.slug,
        "",
        charter,
        "",
    )
}

pub(crate) fn pod_config_template(pod: &PodRef) -> String {
    format!(
        r#"# Orqa pod configuration.
#
# The pod owns backend definitions and the default backend used by fins that do
# not set their own override in fin.toml.
#
# Backend exec_args and chat_args are argv arrays, not shell strings. Supported
# template values
# include:
#   {{orqa_home}}, {{pod}}, {{pod_home}}, {{fin}}, {{fin_home}}, {{home}},
#   {{codex_home}}, {{grok_home}}, {{hermes_home}}, {{mail_home}}, {{task_home}},
#   {{model}}, {{prompt}}

[pod]
slug = "{slug}"
default_backend = "codex"
# Minimum interval between runs for each fin. A fin can override this in
# fin.toml. Examples: "30s", "5m", "3h". Use "0" to run any time there is work.
debounce = "5m"
# Run idle fins at least this often even when they have no mail or tasks. A fin
# can override this in fin.toml. Use "0" to run only when there is work.
exec_always = "0"

# Codex is enabled by default. Adjust command/exec_args/chat_args here if the
# Codex CLI shape changes on this machine.
[backends.codex]
enabled = true
command = "codex"
exec_args = [
    "exec",
    "--skip-git-repo-check",
    "--sandbox", "workspace-write",
    "--cd", "{{pod_home}}",
    "--model", "{{model}}",
    "{{prompt}}",
]
chat_args = [
    "--sandbox", "workspace-write",
    "--cd", "{{pod_home}}",
    "--model", "{{model}}",
]

[backends.codex.defaults]
model = "gpt-5.3-codex"

# Built-in backend definitions are enabled up front. They do nothing unless a
# fin selects them with `backend = "..."`

[backends.opencode]
enabled = true
command = "opencode"
exec_args = ["run", "--model", "{{model}}", "{{prompt}}"]
chat_args = ["--model", "{{model}}"]

[backends.opencode.defaults]
model = "provider/model"

[backends.hermes]
enabled = true
command = "hermes"
exec_args = ["--model", "{{model}}", "--oneshot", "{{prompt}}"]
chat_args = ["chat", "--model", "{{model}}"]

[backends.hermes.defaults]
model = "anthropic/claude-sonnet-4.6"

[backends.pi]
enabled = true
command = "pi"
exec_args = [
    "--model", "{{model}}",
    "--session-dir", "{{fin_home}}/.pi/sessions",
    "--print",
    "{{prompt}}",
]
chat_args = [
    "--model", "{{model}}",
    "--session-dir", "{{fin_home}}/.pi/sessions",
]

[backends.pi.defaults]
model = "provider/model"

# Grok (xAI Grok Build) is a powerful coding agent with strong headless support.
# Use `-p` for single-turn execution and the TUI for interactive chat.
[backends.grok]
enabled = true
command = "grok"
exec_args = ["-p", "{{prompt}}", "--always-approve"]
chat_args = []

[backends.grok.defaults]
model = "grok-code-latest"

# Ollama is most useful through a coding-agent integration. This runs Codex
# against an Ollama model while keeping Codex's tool loop and fin-local
# CODEX_HOME.
[backends.ollama_codex]
enabled = true
command = "ollama"
exec_args = [
    "launch", "codex",
    "--model", "{{model}}",
    "--",
    "exec",
    "--skip-git-repo-check",
    "--sandbox", "workspace-write",
    "--cd", "{{pod_home}}",
    "{{prompt}}",
]
chat_args = [
    "launch", "codex",
    "--model", "{{model}}",
    "--",
    "--sandbox", "workspace-write",
    "--cd", "{{pod_home}}",
]

[backends.ollama_codex.defaults]
model = "gpt-oss:120b"

# [backends.custom]
# enabled = true
# command = "custom-fin-runner"
# exec_args = ["{{prompt}}"]
# chat_args = []
"#,
        slug = pod.slug
    )
}

pub(crate) fn fin_agents_template(fin: &FinRef, role: &str) -> String {
    render_agents_template(
        include_str!("../templates/fin-agents.md"),
        &fin.pod,
        &fin.fin,
        "",
        role,
    )
}

fn render_agents_template(
    template: &str,
    pod: &str,
    fin: &str,
    charter: &str,
    role: &str,
) -> String {
    template
        .replace("{pod}", pod)
        .replace("{fin}", fin)
        .replace("{charter}", charter.trim())
        .replace("{role}", role.trim())
}

#[cfg(test)]
pub(crate) fn fin_config_template(fin: &FinRef) -> String {
    fin_config_template_with_backend(fin, None)
}

pub(crate) fn fin_config_template_with_backend(fin: &FinRef, backend: Option<&str>) -> String {
    let backend_line = backend
        .map(|backend| format!("backend = \"{}\"", escape_toml_string(backend)))
        .unwrap_or_else(|| "# backend = \"codex\"".to_string());

    format!(
        r#"# Orqa fin configuration.
#
# By default a fin inherits the pod default backend from pod.toml.
# Uncomment fin.backend only when this fin should use a different enabled
# backend from its pod.

[fin]
slug = "{slug}"
{backend_line}
# Use "0" to run any time there is work.
# debounce = "5m"
# Use "0" to run only when there is work.
# exec_always = "3h"

# Per-fin template values. These can be used by backend exec_args and chat_args
# in pod.toml.
[backend]
model = "gpt-5.3-codex"
"#,
        slug = fin.fin,
        backend_line = backend_line
    )
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackendCommand {
    pub(crate) backend: String,
    pub(crate) command: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) mode: BackendMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendMode {
    Exec,
    Chat,
}

impl BackendMode {
    fn args_key(self) -> &'static str {
        match self {
            Self::Exec => "exec_args",
            Self::Chat => "chat_args",
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Exec => "exec",
            Self::Chat => "chat",
        }
    }
}

pub(crate) fn backend_command(
    orqa: &Orqa,
    fin: &FinRef,
    prompt_args: &[OsString],
) -> Result<BackendCommand, String> {
    backend_command_for(orqa, fin, prompt_args, BackendMode::Exec)
}

pub(crate) fn backend_chat_command(orqa: &Orqa, fin: &FinRef) -> Result<BackendCommand, String> {
    backend_command_for(orqa, fin, &[], BackendMode::Chat)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RunPolicy {
    pub(crate) debounce: Option<Duration>,
    pub(crate) exec_always: Option<Duration>,
}

pub(crate) fn run_policy(orqa: &Orqa, fin: &FinRef) -> Result<RunPolicy, String> {
    let pod_config = read_toml(
        &orqa
            .pod_data_home(&PodRef::new(&fin.pod)?)?
            .join("pod.toml"),
    )?;
    let fin_config = read_toml(&orqa.fin_data_home(fin)?.join("fin.toml"))?;
    let pod = pod_config.get("pod").and_then(Value::as_table);
    let fin_table = fin_config.get("fin").and_then(Value::as_table);

    Ok(RunPolicy {
        debounce: nonzero_duration(duration_override(pod, fin_table, "debounce")?),
        exec_always: nonzero_duration(duration_override(pod, fin_table, "exec_always")?),
    })
}

fn backend_command_for(
    orqa: &Orqa,
    fin: &FinRef,
    prompt_args: &[OsString],
    mode: BackendMode,
) -> Result<BackendCommand, String> {
    let pod = PodRef::new(&fin.pod)?;
    let pod_config = read_toml(&orqa.pod_data_home(&pod)?.join("pod.toml"))?;
    let fin_config = read_toml(&orqa.fin_data_home(fin)?.join("fin.toml"))?;
    let backend_name =
        fin_backend(&fin_config)?.unwrap_or_else(|| pod_default_backend(&pod_config));
    let backend = backend_table(&pod_config, &backend_name)?;

    if !bool_field(backend, "enabled").unwrap_or(false) {
        return Err(format!("backend {backend_name:?} is not enabled"));
    }

    let command = string_field(backend, "command")
        .ok_or_else(|| format!("backend {backend_name:?} is missing command"))?;
    let backend_args = required_string_array_field(backend, mode.args_key())?;
    let values = backend_values(orqa, fin, prompt_args, backend, &fin_config)?;
    let args = backend_args
        .iter()
        .map(|arg| OsString::from(expand_templates(arg, &values)))
        .collect();

    Ok(BackendCommand {
        backend: backend_name,
        command: OsString::from(command),
        args,
        mode,
    })
}

fn read_toml(path: &std::path::Path) -> Result<Table, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read config {}: {error}", path.display()))?;
    contents
        .parse::<Table>()
        .map_err(|error| format!("failed to parse config {}: {error}", path.display()))
}

fn pod_default_backend(pod_config: &Table) -> String {
    pod_config
        .get("pod")
        .and_then(Value::as_table)
        .and_then(|pod| string_field(pod, "default_backend"))
        .unwrap_or_else(|| "codex".to_string())
}

fn fin_backend(fin_config: &Table) -> Result<Option<String>, String> {
    Ok(fin_config
        .get("fin")
        .and_then(Value::as_table)
        .and_then(|fin| string_field(fin, "backend")))
}

fn backend_table<'a>(pod_config: &'a Table, backend_name: &str) -> Result<&'a Table, String> {
    pod_config
        .get("backends")
        .and_then(Value::as_table)
        .and_then(|backends| backends.get(backend_name))
        .and_then(Value::as_table)
        .ok_or_else(|| format!("backend {backend_name:?} is not defined in pod.toml"))
}

fn backend_values(
    orqa: &Orqa,
    fin: &FinRef,
    prompt_args: &[OsString],
    backend: &Table,
    fin_config: &Table,
) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    let pod = PodRef::new(&fin.pod)?;
    let pod_home = orqa.pod_data_home(&pod)?;
    let fin_home = orqa.fin_data_home(fin)?;

    values.insert("orqa_home".to_string(), orqa.home.display().to_string());
    values.insert("pod".to_string(), fin.pod.clone());
    values.insert("pod_home".to_string(), pod_home.display().to_string());
    values.insert("fin".to_string(), fin.fin.clone());
    values.insert("fin_home".to_string(), fin_home.display().to_string());
    values.insert(
        "codex_home".to_string(),
        fin_home.join(".codex").display().to_string(),
    );
    values.insert(
        "grok_home".to_string(),
        fin_home.join(".grok").display().to_string(),
    );
    values.insert(
        "hermes_home".to_string(),
        fin_home.join(".hermes").display().to_string(),
    );
    values.insert("home".to_string(), fin_home.display().to_string());
    values.insert(
        "mail_home".to_string(),
        orqa.mail_home(fin)?.display().to_string(),
    );
    values.insert(
        "task_home".to_string(),
        orqa.task_home(fin)?.display().to_string(),
    );
    values.insert("prompt".to_string(), prompt_args_to_string(prompt_args));

    if let Some(defaults) = backend.get("defaults").and_then(Value::as_table) {
        values.extend(string_values(defaults));
    }
    if let Some(fin_backend) = fin_config.get("backend").and_then(Value::as_table) {
        values.extend(string_values(fin_backend));
    }

    Ok(values)
}

fn string_values(table: &Table) -> BTreeMap<String, String> {
    table
        .iter()
        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
        .collect()
}

fn prompt_args_to_string(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn expand_templates(input: &str, values: &BTreeMap<String, String>) -> String {
    let mut expanded = input.to_string();
    for (key, value) in values {
        expanded = expanded.replace(&format!("{{{key}}}"), value);
    }
    expanded
}

fn string_field(table: &Table, key: &str) -> Option<String> {
    table.get(key)?.as_str().map(str::to_string)
}

fn bool_field(table: &Table, key: &str) -> Option<bool> {
    table.get(key)?.as_bool()
}

fn duration_override(
    pod: Option<&Table>,
    fin: Option<&Table>,
    key: &str,
) -> Result<Option<Duration>, String> {
    match fin
        .and_then(|table| string_field(table, key))
        .or_else(|| pod.and_then(|table| string_field(table, key)))
    {
        Some(value) => parse_duration(&value)
            .map(Some)
            .map_err(|error| format!("invalid {key} {value:?}: {error}")),
        None => Ok(None),
    }
}

fn nonzero_duration(duration: Option<Duration>) -> Option<Duration> {
    duration.filter(|duration| !duration.is_zero())
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("duration cannot be empty".to_string());
    }

    let split = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let number = value[..split]
        .parse::<u64>()
        .map_err(|_| "duration must start with a positive integer".to_string())?;
    let unit = value[split..].trim().to_ascii_lowercase();
    let seconds = match unit.as_str() {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => number,
        "m" | "min" | "mins" | "minute" | "minutes" => number * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => number * 60 * 60,
        "d" | "day" | "days" => number * 60 * 60 * 24,
        _ => return Err("use a duration like 30s, 5m, 3h, or 1 day".to_string()),
    };
    Ok(Duration::from_secs(seconds))
}

fn required_string_array_field(table: &Table, key: &str) -> Result<Vec<String>, String> {
    let Some(value) = table.get(key) else {
        return Err(format!("{key} must be defined as an array of strings"));
    };
    let Some(array) = value.as_array() else {
        return Err(format!("{key} must be an array of strings"));
    };

    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{key} must be an array of strings"))
        })
        .collect()
}

#[cfg(test)]
#[path = "config_test.rs"]
mod tests;

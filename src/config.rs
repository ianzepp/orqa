use std::{collections::BTreeMap, ffi::OsString, fs};

use toml::{Table, Value};

use crate::model::{FinRef, Orqa, PodRef};

pub(crate) fn pod_config_template(pod: &PodRef) -> String {
    format!(
        r#"# Orqa pod configuration.
#
# The pod owns backend definitions and the default backend used by fins that do
# not set their own override in fin.toml.
#
# Backend args are argv arrays, not shell strings. Supported template values
# include:
#   {{orqa_home}}, {{pod}}, {{pod_home}}, {{fin}}, {{fin_home}}, {{codex_home}},
#   {{mail_home}}, {{task_home}}, {{model}}, {{prompt}}

[pod]
slug = "{slug}"
default_backend = "codex"

# Codex is enabled by default. Adjust command/args here if the Codex CLI shape
# changes on this machine.
[backends.codex]
enabled = true
command = "codex"
args = ["{{prompt}}"]

[backends.codex.defaults]
model = "gpt-5.3-codex"

# Enable and edit these examples if this pod should allow additional backends.

# [backends.opencode]
# enabled = true
# command = "opencode"
# args = ["run", "--model", "{{model}}", "{{prompt}}"]
#
# [backends.opencode.defaults]
# model = "default"

# [backends.pi]
# enabled = true
# command = "pi"
# args = [
#     "exec",
#     "--home", "{{fin_home}}",
#     "--pod", "{{pod}}",
#     "--fin", "{{fin}}",
#     "{{prompt}}",
# ]

# [backends.custom]
# enabled = true
# command = "custom-fin-runner"
# args = ["{{prompt}}"]
"#,
        slug = pod.slug
    )
}

pub(crate) fn fin_config_template(fin: &FinRef) -> String {
    format!(
        r#"# Orqa fin configuration.
#
# By default a fin inherits the pod default backend from pod.toml.
# Uncomment fin.backend only when this fin should use a different enabled
# backend from its pod.

[fin]
slug = "{slug}"
# backend = "codex"

# Per-fin template values. These can be used by backend args in pod.toml.
[backend]
model = "gpt-5.3-codex"
"#,
        slug = fin.fin
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackendCommand {
    pub(crate) backend: String,
    pub(crate) command: OsString,
    pub(crate) args: Vec<OsString>,
}

pub(crate) fn backend_command(
    orqa: &Orqa,
    fin: &FinRef,
    prompt_args: &[OsString],
) -> Result<BackendCommand, String> {
    let pod_config = read_toml(&orqa.pod_home(&PodRef::new(&fin.pod)?).join("pod.toml"))?;
    let fin_config = read_toml(&orqa.fin_home(fin).join("fin.toml"))?;
    let backend_name =
        fin_backend(&fin_config)?.unwrap_or_else(|| pod_default_backend(&pod_config));
    let backend = backend_table(&pod_config, &backend_name)?;

    if !bool_field(backend, "enabled").unwrap_or(false) {
        return Err(format!("backend {backend_name:?} is not enabled"));
    }

    let command = string_field(backend, "command")
        .ok_or_else(|| format!("backend {backend_name:?} is missing command"))?;
    let backend_args = string_array_field(backend, "args")?;
    let values = backend_values(orqa, fin, prompt_args, backend, &fin_config)?;
    let args = backend_args
        .iter()
        .map(|arg| OsString::from(expand_templates(arg, &values)))
        .collect();

    Ok(BackendCommand {
        backend: backend_name,
        command: OsString::from(command),
        args,
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
    let fin_home = orqa.fin_home(fin);

    values.insert("orqa_home".to_string(), orqa.home.display().to_string());
    values.insert("pod".to_string(), fin.pod.clone());
    values.insert(
        "pod_home".to_string(),
        orqa.pod_home(&pod).display().to_string(),
    );
    values.insert("fin".to_string(), fin.fin.clone());
    values.insert("fin_home".to_string(), fin_home.display().to_string());
    values.insert(
        "codex_home".to_string(),
        fin_home.join(".codex").display().to_string(),
    );
    values.insert(
        "mail_home".to_string(),
        orqa.mail_home(fin).display().to_string(),
    );
    values.insert(
        "task_home".to_string(),
        orqa.task_home(fin).display().to_string(),
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

fn string_array_field(table: &Table, key: &str) -> Result<Vec<String>, String> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
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

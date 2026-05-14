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
use crate::model::{FinRef, PodRef};

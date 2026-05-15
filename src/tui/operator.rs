//! Safe provisioning of the local `operator` fin for the TUI cockpit.
//!
//! This fin is the human operator's dedicated identity inside a pod. It is
//! created automatically the first time the TUI is launched inside a valid
//! Phase 05 pod root. It is never created by `orqa init` or `pod create`.
//!
//! Safety rules (enforced here):
//! - Only called after successful pod detection (we have a real pod_root).
//! - Never creates the pod itself.
//! - Idempotent — safe to call on every TUI launch.

use std::{fs, path::Path};

use crate::model::{Orqa, PodRegistration};

/// Ensures the special `operator` fin exists under the given pod root's
/// `.orqa/fins/operator/` directory.
///
/// Returns `Ok(())` if the fin already existed or was successfully created.
/// Returns `Err` only on real I/O or permission problems.
pub fn ensure_operator_fin(orqa: &Orqa, pod_slug: &str, pod_root: &Path) -> Result<(), String> {
    let reg = PodRegistration {
        slug: pod_slug.to_string(),
        path: pod_root.to_path_buf(),
        enabled: true,
    };

    let fin_home = orqa.fin_data_home(&reg, "operator");

    // If fin.toml already exists, we consider the fin provisioned.
    if fin_home.join("fin.toml").exists() {
        return Ok(());
    }

    // Create the directory structure
    fs::create_dir_all(fin_home.join("mail").join("new"))
        .map_err(|e| format!("failed to create operator fin mail/new: {e}"))?;
    fs::create_dir_all(fin_home.join("mail").join("cur"))
        .map_err(|e| format!("failed to create operator fin mail/cur: {e}"))?;
    fs::create_dir_all(fin_home.join("mail").join("tmp"))
        .map_err(|e| format!("failed to create operator fin mail/tmp: {e}"))?;

    fs::create_dir_all(fin_home.join("tasks").join("new"))
        .map_err(|e| format!("failed to create operator fin tasks/new: {e}"))?;
    fs::create_dir_all(fin_home.join("tasks").join("cur"))
        .map_err(|e| format!("failed to create operator fin tasks/cur: {e}"))?;
    fs::create_dir_all(fin_home.join("tasks").join("tmp"))
        .map_err(|e| format!("failed to create operator fin tasks/tmp: {e}"))?;

    fs::create_dir_all(fin_home.join("runs"))
        .map_err(|e| format!("failed to create operator fin runs: {e}"))?;

    // Write the minimal identity files
    let fin_toml = format!(
        "# Operator fin for the human TUI cockpit in pod '{pod_slug}'.\n\
         # This fin exists only so the human has a stable identity (`operator@{pod_slug}.orqa`)\n\
         # when using the `orqa` TUI inside this project.\n\n\
         [fin]\n\
         slug = \"operator\"\n"
    );
    fs::write(fin_home.join("fin.toml"), fin_toml)
        .map_err(|e| format!("failed to write operator fin.toml: {e}"))?;

    let role_md = "\
# Operator

This is the dedicated identity for the human operator using the `orqa` TUI cockpit
inside this pod.

- You (the human) interact with the rest of the pod through this identity.
- Other fins should mail `operator@$ORQA_POD.orqa` when they need human attention
  or have results to report.
- The TUI is the primary (and currently only) way this fin is \"run\".

This fin is intentionally excluded from normal background wake-loop scheduling.
";
    fs::write(fin_home.join("ROLE.md"), role_md)
        .map_err(|e| format!("failed to write operator ROLE.md: {e}"))?;

    let agents_md = "\
# Orqa Fin Instructions

You are the `operator` fin in the `{pod}` pod.

This identity exists so the human has a stable address (`operator@{pod}.orqa`)
when using the interactive TUI cockpit for this pod.

## Role

Human operator surface. You receive escalations and questions from other fins
via pod-local mail and can send directives back to them.

## Operating Notes

- The human primarily drives you via the `orqa` TUI (not via the normal wake loop).
- When the human sends you mail through the TUI composer, process it as a high-priority
  request from the operator.
- Reply to the human by mailing `operator@{pod}.orqa` (the same local inbox the TUI watches).
";
    fs::write(fin_home.join("AGENTS.md"), agents_md)
        .map_err(|e| format!("failed to write operator AGENTS.md: {e}"))?;

    fs::write(
        fin_home.join("fin.txt"),
        format!("slug=operator\npod={pod_slug}\n"),
    )
    .map_err(|e| format!("failed to write operator fin.txt: {e}"))?;

    // Create an empty latest-run pointer so status/tail commands don't explode
    let _ = fs::write(fin_home.join("latest-run"), "");

    Ok(())
}

//! Pod command handlers (create, list, charter, status, doctor, hooks, tail, sleep/wake, init).
//!
//! Extracted from commands/mod.rs as part of the large-module split.

use super::list_dirs;
use crate::model::{Orqa, PodRef};
use crate::status::{pod_status, print_pod_list_status};

pub(crate) fn list_pods(orqa: &Orqa) -> Result<(), String> {
    // Phase 05-3+: Prefer registry (new pod roots) — basic listing for now
    let regs = crate::model::load_registry(orqa).unwrap_or_default();
    if !regs.is_empty() {
        for reg in regs.values().filter(|r| r.enabled) {
            println!("- {} @ {}", reg.slug, reg.path.display());
        }
    } else {
        // Legacy fallback
        for pod in list_dirs(&orqa.home.join("pods"))? {
            let pod = PodRef::new(&pod)?;
            print_pod_list_status(&pod_status(orqa, &pod)?);
        }
    }
    Ok(())
}

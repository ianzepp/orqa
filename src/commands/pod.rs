//! Pod command handlers (create, list, charter, status, doctor, hooks, tail, sleep/wake, init).

use crate::model::{Orqa, PodRef, load_registry};
use crate::status::{pod_status, print_pod_list_status};

pub(crate) fn list_pods(orqa: &Orqa) -> Result<(), String> {
    for reg in load_registry(orqa)?.values().filter(|reg| reg.enabled) {
        let pod = PodRef::new(&reg.slug)?;
        print_pod_list_status(&pod_status(orqa, &pod)?);
    }
    Ok(())
}

use std::{ffi::OsString, thread, time::Duration};

use crate::{
    model::{Orqa, load_registry},
    runtime::{wake_pod, wake_pod_quiet},
};

pub(crate) const DEFAULT_GLOBAL_LOOP_INTERVAL: u64 = 10;
pub(crate) const DEFAULT_GLOBAL_LOOP_PROMPT: &str = "handle your open Orqa mail and tasks";

pub(crate) fn wake_all_pods(
    orqa: &Orqa,
    args: &[OsString],
    quiet: bool,
) -> Result<Vec<(String, Result<(), String>)>, String> {
    let registry = load_registry(orqa)?;
    let mut results = Vec::new();

    for reg in registry.values().filter(|reg| reg.enabled) {
        let result = if quiet {
            wake_pod_quiet(orqa, &reg.slug, false, false, false, args)
        } else {
            wake_pod(orqa, &reg.slug, false, false, false, args)
        };
        results.push((reg.slug.clone(), result));
    }

    Ok(results)
}

pub(crate) fn run_daemon(orqa: &Orqa, interval: u64, args: Vec<OsString>) -> Result<(), String> {
    if interval == 0 {
        return Err("daemon interval must be at least 1 second".to_string());
    }

    let args = if args.is_empty() {
        vec![OsString::from(DEFAULT_GLOBAL_LOOP_PROMPT)]
    } else {
        args
    };

    loop {
        println!("orqa daemon wake");
        for (pod, result) in wake_all_pods(orqa, &args, false)? {
            if let Err(error) = result {
                eprintln!("pod {pod} wake error: {error}");
            }
        }
        thread::sleep(Duration::from_secs(interval));
    }
}

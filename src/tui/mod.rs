//! TUI Operator Cockpit for project-root pods.
//!
//! This module contains the Ratatui-based live monitoring + operator injection
//! surface. It is only entered when `resolve_pod_context` successfully detects
//! a pod root containing `.orqa/pod.toml`.

pub mod app;
pub mod composer;
pub mod events;
pub mod loopctl;
pub mod run;
pub mod theme;
pub mod top;
#[cfg(test)]
mod top_test;
mod view;
pub mod watcher;

pub use run::run_tui;
pub use top::run_top;

#[allow(unused_imports)]
pub use watcher::PodWatcher;

// Event will be publicly re-exported once the timeline renderer consumes it.

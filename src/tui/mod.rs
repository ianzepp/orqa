//! TUI Operator Cockpit for orqa (Phase 05+ pod roots).
//!
//! This module contains the Ratatui-based live monitoring + operator injection
//! surface. It is only entered when `resolve_pod_context` successfully detects
//! a pod root containing `.orqa/pod.toml`.

pub mod app;
pub mod composer;
pub mod events;
pub mod operator;
pub mod run;
pub mod theme;
pub mod watcher;

pub use operator::ensure_operator_fin;
pub use run::run_tui;

#[allow(unused_imports)]
pub use watcher::PodWatcher;

// Event will be publicly re-exported once the timeline renderer (Phase 3) consumes it.

//! TUI Operator Cockpit for orqa (Phase 05+ pod roots).
//!
//! This module contains the Ratatui-based live monitoring + operator injection
//! surface. It is only entered when `resolve_pod_context` successfully detects
//! a pod root containing `.orqa/pod.toml`.

pub mod operator;
pub mod run;

pub use operator::ensure_operator_fin;
pub use run::run_tui;

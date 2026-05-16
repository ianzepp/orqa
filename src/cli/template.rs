use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(crate) struct TemplateCommand {
    #[command(subcommand)]
    pub(crate) command: TemplateSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TemplateSubcommand {
    /// List installed pod templates.
    List,
    /// Create a pod and seed fins from a template.
    #[command(name = "create-pod")]
    CreatePod(TemplateCreatePodArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TemplateCreatePodArgs {
    /// Template slug under ORQA_HOME/templates.
    pub(crate) template: String,
    /// Pod slug to create.
    pub(crate) slug: String,
    /// Create the pod rooted in this directory.
    #[arg(long, value_name = "DIR")]
    pub(crate) path: Option<PathBuf>,
    /// Pod charter text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) charter: Option<String>,
}

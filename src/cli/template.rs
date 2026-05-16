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
    /// Create an empty pod template.
    Create(TemplateCreateArgs),
    /// Manage fins inside a pod template.
    Fin(TemplateFinCommand),
    /// Create a pod and seed fins from a template.
    #[command(name = "create-pod")]
    CreatePod(TemplateCreatePodArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TemplateCreateArgs {
    /// Template slug under ORQA_HOME/templates.
    pub(crate) template: String,
}

#[derive(Debug, Args)]
pub(crate) struct TemplateFinCommand {
    #[command(subcommand)]
    pub(crate) command: TemplateFinSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TemplateFinSubcommand {
    /// List fins defined in a pod template.
    List(TemplateFinListArgs),
    /// Add a fin role to a pod template.
    Create(TemplateFinCreateArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TemplateFinListArgs {
    /// Template slug under ORQA_HOME/templates.
    pub(crate) template: String,
}

#[derive(Debug, Args)]
pub(crate) struct TemplateFinCreateArgs {
    /// Template slug under ORQA_HOME/templates.
    pub(crate) template: String,
    /// Fin slug inside the template.
    pub(crate) fin: String,
    /// Fin role text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) role: String,
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

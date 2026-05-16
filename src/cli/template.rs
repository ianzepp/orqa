use clap::{Args, Subcommand};

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

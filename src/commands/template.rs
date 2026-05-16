use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    cli::{TemplateCommand, TemplateFinSubcommand, TemplateSubcommand},
    model::{Orqa, validate_slug},
};

use super::{list_dirs, read_markdown_source};

pub(crate) fn template(orqa: &Orqa, command: TemplateCommand) -> Result<(), String> {
    match command.command {
        TemplateSubcommand::List => list_templates(orqa),
        TemplateSubcommand::Create(args) => create_template(orqa, &args.template),
        TemplateSubcommand::Fin(command) => match command.command {
            TemplateFinSubcommand::List(args) => list_template_fins(orqa, &args.template),
            TemplateFinSubcommand::Create(args) => {
                create_template_fin(orqa, &args.template, &args.fin, &args.role)
            }
        },
    }
}

fn create_template(orqa: &Orqa, template: &str) -> Result<(), String> {
    validate_slug(template)?;
    let fins_dir = template_home(orqa, template).join("fins");
    fs::create_dir_all(&fins_dir).map_err(|error| {
        format!(
            "failed to create template directory {}: {error}",
            fins_dir.display()
        )
    })?;
    println!("{}", template_home(orqa, template).display());
    Ok(())
}

fn list_template_fins(orqa: &Orqa, template: &str) -> Result<(), String> {
    validate_slug(template)?;
    let template_dir = template_home(orqa, template);
    let fins_dir = template_fins_dir(&template_dir)?;
    super::print_dirs(&fins_dir)
}

fn create_template_fin(
    orqa: &Orqa,
    template: &str,
    fin: &str,
    role_source: &str,
) -> Result<(), String> {
    validate_slug(template)?;
    validate_slug(fin)?;
    if fin == "operator" {
        return Err(
            "template fins may not include 'operator'; pods seed that local human fin automatically"
                .to_string(),
        );
    }

    let template_dir = template_home(orqa, template);
    if !template_dir.exists() {
        return Err(format!(
            "template '{}' does not exist (run 'orqa template create {}' first)",
            template, template
        ));
    }

    let fins_dir = template_fins_dir(&template_dir)?;
    let fin_dir = fins_dir.join(fin);
    let role_path = fin_dir.join("ROLE.md");
    if role_path.exists() {
        return Err(format!(
            "template fin '{}' already exists at {}",
            fin,
            fin_dir.display()
        ));
    }

    fs::create_dir_all(&fin_dir).map_err(|error| {
        format!(
            "failed to create template fin {}: {error}",
            fin_dir.display()
        )
    })?;
    let role = read_markdown_source(role_source)?;
    fs::write(&role_path, role)
        .map_err(|error| format!("failed to write {}: {error}", role_path.display()))?;

    println!("{}", fin_dir.display());
    Ok(())
}

fn list_templates(orqa: &Orqa) -> Result<(), String> {
    let templates_dir = templates_home(orqa);
    fs::create_dir_all(&templates_dir).map_err(|error| {
        format!(
            "failed to create templates directory {}: {error}",
            templates_dir.display()
        )
    })?;
    super::print_dirs(&templates_dir)
}

fn templates_home(orqa: &Orqa) -> PathBuf {
    orqa.home.join("templates")
}

pub(super) fn template_home(orqa: &Orqa, template: &str) -> PathBuf {
    templates_home(orqa).join(template)
}

pub(super) fn template_fins_dir(template_dir: &Path) -> Result<PathBuf, String> {
    if !template_dir.exists() {
        return Err(format!(
            "template '{}' does not exist",
            template_dir.display()
        ));
    }

    let pod_style = template_dir.join(".orqa").join("fins");
    if pod_style.is_dir() {
        return Ok(pod_style);
    }

    let compact_style = template_dir.join("fins");
    if compact_style.is_dir() {
        return Ok(compact_style);
    }

    Err(format!(
        "template '{}' must contain .orqa/fins or fins",
        template_dir.display()
    ))
}

pub(super) struct TemplateFin {
    pub(super) slug: String,
    pub(super) role_path: PathBuf,
}

pub(super) fn template_fins(fins_dir: &Path) -> Result<Vec<TemplateFin>, String> {
    let mut fins = Vec::new();
    for slug in list_dirs(fins_dir)? {
        validate_slug(&slug)?;
        let role_path = fins_dir.join(&slug).join("ROLE.md");
        if !role_path.is_file() {
            return Err(format!(
                "template fin '{}' is missing {}",
                slug,
                role_path.display()
            ));
        }
        fins.push(TemplateFin { slug, role_path });
    }
    Ok(fins)
}

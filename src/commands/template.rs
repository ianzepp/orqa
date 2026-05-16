use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    cli::{TemplateCommand, TemplateSubcommand},
    model::{Orqa, validate_slug},
};

use super::{create_pod_in_dir, fin::create_fin_in_pod, list_dirs};

pub(crate) fn template(orqa: &Orqa, command: TemplateCommand) -> Result<(), String> {
    match command.command {
        TemplateSubcommand::List => list_templates(orqa),
        TemplateSubcommand::CreatePod(args) => {
            validate_slug(&args.template)?;
            validate_slug(&args.slug)?;

            let template_dir = template_home(orqa, &args.template);
            let fins_dir = template_fins_dir(&template_dir)?;
            let fins = template_fins(&fins_dir)?;
            if fins.is_empty() {
                return Err(format!(
                    "template '{}' has no fins under {}",
                    args.template,
                    fins_dir.display()
                ));
            }
            if fins.iter().any(|fin| fin.slug == "operator") {
                return Err(
                    "template fins may not include 'operator'; pods seed that local human fin automatically"
                        .to_string(),
                );
            }

            let target_root = match args.path {
                Some(path) => path,
                None => std::env::current_dir()
                    .map_err(|error| format!("failed to get current directory: {error}"))?,
            };

            create_pod_in_dir(orqa, &args.slug, target_root.clone(), args.charter)?;

            for fin in fins {
                let role_arg = format!("@{}", fin.role_path.display());
                create_fin_in_pod(
                    orqa,
                    &args.slug,
                    &target_root,
                    &fin.slug,
                    Some(&role_arg),
                    None,
                )?;
            }

            println!(
                "Created pod '{}' from template '{}'",
                args.slug, args.template
            );
            Ok(())
        }
    }
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

fn template_home(orqa: &Orqa, template: &str) -> PathBuf {
    templates_home(orqa).join(template)
}

fn template_fins_dir(template_dir: &Path) -> Result<PathBuf, String> {
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

struct TemplateFin {
    slug: String,
    role_path: PathBuf,
}

fn template_fins(fins_dir: &Path) -> Result<Vec<TemplateFin>, String> {
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

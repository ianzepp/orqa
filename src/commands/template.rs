use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    cli::{CommandContext, TemplateCommand, TemplateFinSubcommand, TemplateSubcommand},
    config::{fin_agents_template, fin_config_template_with_backend_and_template},
    mailbox::{ensure_maildir, write_if_missing},
    model::{FinRef, Orqa, PodRef, validate_slug},
    runtime_home::ensure_fin_runtime_homes,
};

use super::{list_dirs, read_markdown_source, write_text};

pub(crate) fn template(
    orqa: &Orqa,
    context: &CommandContext,
    command: TemplateCommand,
) -> Result<(), String> {
    match command.command {
        TemplateSubcommand::List => list_templates(orqa),
        TemplateSubcommand::Create(args) => create_template(orqa, &args.template),
        TemplateSubcommand::Sync(args) => {
            sync_template(orqa, context, &args.template, args.dry_run)
        }
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

    let names = super::list_dirs(&fins_dir)?;
    if names.is_empty() {
        println!("No fins defined in template '{}'.", template);
        println!();
        println!(
            "Add one with: orqa template fin create {} <fin> --role <prompt|@file|->",
            template
        );
        return Ok(());
    }

    for name in names {
        println!("{name}");
    }
    Ok(())
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

fn sync_template(
    orqa: &Orqa,
    context: &CommandContext,
    template: &str,
    dry_run: bool,
) -> Result<(), String> {
    validate_slug(template)?;
    let (pod_slug, pod_root) = context.resolve_pod(None, orqa)?;
    let pod = PodRef::new(&pod_slug)?;
    orqa.ensure_pod_exists(&pod)?;

    let (_, template_fins) = load_template_fins_for_sync(orqa, template)?;
    let template_slugs: BTreeSet<String> =
        template_fins.iter().map(|fin| fin.slug.clone()).collect();
    let template_by_slug: BTreeMap<String, PathBuf> = template_fins
        .into_iter()
        .map(|fin| (fin.slug, fin.role_path))
        .collect();
    let fins_dir = pod_root.join(".orqa").join("fins");
    let existing_fins = list_dirs(&fins_dir)?;
    let mut changed = false;

    println!(
        "Sync template '{}' into pod '{}'{}",
        template,
        pod_slug,
        if dry_run { " (dry run)" } else { "" }
    );

    for (fin_slug, role_path) in &template_by_slug {
        let fin_home = fins_dir.join(fin_slug);
        let role = fs::read_to_string(role_path)
            .map_err(|error| format!("failed to read {}: {error}", role_path.display()))?;
        let fin = FinRef::new(&pod_slug, fin_slug)?;

        if !fin_home.join("fin.toml").exists() {
            changed = true;
            println!("ADD fin {fin_slug}");
            println!("  + {}", fin_home.join("fin.toml").display());
            println!("  + {}", fin_home.join("ROLE.md").display());
            println!("  + {}", fin_home.join("AGENTS.md").display());
            if !dry_run {
                create_fin_from_template(orqa, &pod_slug, &pod_root, template, fin_slug, &role)?;
            }
            continue;
        }

        match read_template_origin(&fin_home.join("fin.toml"))? {
            Some(origin) if origin != template => {
                println!(
                    "SKIP fin {fin_slug}: owned by template '{}', not '{}'",
                    origin, template
                );
                continue;
            }
            Some(_) => {}
            None => {
                changed = true;
                println!("ADOPT fin {fin_slug}");
                println!("  ~ {}", fin_home.join("fin.toml").display());
                if !dry_run {
                    write_template_origin(&fin_home.join("fin.toml"), template, fin_slug)?;
                }
            }
        }

        let agents = fin_agents_template(&fin, &role);
        let role_needs_update = file_contents(&fin_home.join("ROLE.md"))? != role;
        let agents_needs_update = file_contents(&fin_home.join("AGENTS.md"))? != agents;

        if role_needs_update {
            changed = true;
            println!("UPDATE fin {fin_slug}");
            println!("  ~ {}", fin_home.join("ROLE.md").display());
            if !dry_run {
                write_text(&fin_home.join("ROLE.md"), &role)?;
            }
        }
        if agents_needs_update {
            changed = true;
            if !role_needs_update {
                println!("UPDATE fin {fin_slug}");
            }
            println!("  ~ {}", fin_home.join("AGENTS.md").display());
            if !dry_run {
                write_text(&fin_home.join("AGENTS.md"), &agents)?;
            }
        }
    }

    for fin_slug in existing_fins {
        if fin_slug == "operator" || template_slugs.contains(&fin_slug) {
            continue;
        }
        let fin_home = fins_dir.join(&fin_slug);
        if read_template_origin(&fin_home.join("fin.toml"))?.as_deref() == Some(template) {
            changed = true;
            println!(
                "DELETE fin {fin_slug}: removed from template '{}' and template-owned",
                template
            );
            println!("  ! {}", fin_home.display());
            if !dry_run {
                fs::remove_dir_all(&fin_home)
                    .map_err(|error| format!("failed to delete {}: {error}", fin_home.display()))?;
            }
        }
    }

    if !changed {
        println!("No template changes.");
    }
    Ok(())
}

pub(super) fn create_fin_from_template(
    orqa: &Orqa,
    pod_slug: &str,
    pod_root: &Path,
    template: &str,
    fin_slug: &str,
    role: &str,
) -> Result<(), String> {
    let fin = FinRef::new(pod_slug, fin_slug)?;
    let fin_home = pod_root.join(".orqa").join("fins").join(fin_slug);
    ensure_fin_runtime_homes(orqa, &fin)?;
    ensure_maildir(&fin_home.join("mail"))?;
    ensure_maildir(&fin_home.join("tasks"))?;

    write_if_missing(&fin_home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
    write_if_missing(
        &fin_home.join("fin.toml"),
        &fin_config_template_with_backend_and_template(&fin, None, Some(template)),
    )?;
    write_if_missing(&fin_home.join("ROLE.md"), role)?;
    write_if_missing(
        &fin_home.join("AGENTS.md"),
        &fin_agents_template(&fin, role),
    )?;

    println!("{}", fin_home.display());
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

    let names = super::list_dirs(&templates_dir)?;
    if names.is_empty() {
        println!("No templates found.");
        println!();
        println!("Create one with: orqa template create <name>");
        return Ok(());
    }

    for name in names {
        let template_dir = template_home(orqa, &name);
        let fins_dir = template_fins_dir(&template_dir)?;
        let fins: Vec<String> = template_fins(&fins_dir)?
            .into_iter()
            .map(|fin| fin.slug)
            .collect();
        if fins.is_empty() {
            println!("{name} fins=0");
        } else {
            println!("{name} fins={} [{}]", fins.len(), fins.join(", "));
        }
    }
    Ok(())
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

fn load_template_fins_for_sync(
    orqa: &Orqa,
    template: &str,
) -> Result<(String, Vec<TemplateFin>), String> {
    validate_slug(template)?;
    let template_dir = template_home(orqa, template);
    let fins_dir = template_fins_dir(&template_dir)?;
    let fins = template_fins(&fins_dir)?;
    if fins.is_empty() {
        return Err(format!(
            "template '{}' has no fins under {}",
            template,
            fins_dir.display()
        ));
    }
    if fins.iter().any(|fin| fin.slug == "operator") {
        return Err(
            "template fins may not include 'operator'; pods seed that local human fin automatically"
                .to_string(),
        );
    }

    Ok((template.to_string(), fins))
}

fn file_contents(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn read_template_origin(fin_toml: &Path) -> Result<Option<String>, String> {
    if !fin_toml.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(fin_toml)
        .map_err(|error| format!("failed to read {}: {error}", fin_toml.display()))?;
    let parsed = contents
        .parse::<toml::Table>()
        .map_err(|error| format!("failed to parse {}: {error}", fin_toml.display()))?;
    Ok(parsed
        .get("template")
        .and_then(toml::Value::as_table)
        .and_then(|template| template.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_string))
}

fn write_template_origin(fin_toml: &Path, template: &str, fin: &str) -> Result<(), String> {
    let mut contents = fs::read_to_string(fin_toml)
        .map_err(|error| format!("failed to read {}: {error}", fin_toml.display()))?;
    if contents.parse::<toml::Table>().is_err() {
        return Err(format!("failed to parse {}", fin_toml.display()));
    }
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&format!(
        r#"
[template]
name = "{}"
fin = "{}"
"#,
        escape_toml_string(template),
        escape_toml_string(fin)
    ));
    fs::write(fin_toml, contents)
        .map_err(|error| format!("failed to write {}: {error}", fin_toml.display()))
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

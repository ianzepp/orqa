use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    cli::{CommandContext, TemplateCommand, TemplateFinSubcommand, TemplateSubcommand},
    config::{
        fin_agents_template, fin_config_template_with_backend_and_template,
        template_fin_config_template,
    },
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
    let agents_path = fin_dir.join("AGENTS.md");
    if agents_path.exists() || fin_dir.join("ROLE.md").exists() {
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
    let agents = read_markdown_source(role_source)?;
    fs::write(&agents_path, agents)
        .map_err(|error| format!("failed to write {}: {error}", agents_path.display()))?;
    let config_path = fin_dir.join("fin.toml");
    fs::write(&config_path, template_fin_config_template(fin))
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;

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
    let template_by_slug: BTreeMap<String, TemplateFin> = template_fins
        .into_iter()
        .map(|fin| (fin.slug.clone(), fin))
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

    if sync_template_pod_agents(orqa, &pod_root, template, dry_run)? {
        changed = true;
    }

    for (fin_slug, template_fin) in &template_by_slug {
        let fin_home = fins_dir.join(fin_slug);
        let template_agents = fs::read_to_string(&template_fin.agents_path).map_err(|error| {
            format!(
                "failed to read {}: {error}",
                template_fin.agents_path.display()
            )
        })?;
        let config = read_template_fin_config(template_fin)?;
        let fin = FinRef::new(&pod_slug, fin_slug)?;

        if !fin_home.join("fin.toml").exists() {
            changed = true;
            println!("ADD fin {fin_slug}");
            println!("  + {}", fin_home.join("fin.toml").display());
            println!("  + {}", fin_home.join("ROLE.md").display());
            println!("  + {}", fin_home.join("AGENTS.md").display());
            if !dry_run {
                create_fin_from_template(
                    orqa,
                    &pod_slug,
                    &pod_root,
                    template,
                    fin_slug,
                    &template_agents,
                    config.as_deref(),
                )?;
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

        let agents = fin_agents_template(&fin, &template_agents);
        let desired_config = config
            .as_deref()
            .map(|config| materialized_fin_config(&fin, template, Some(config)))
            .transpose()?;
        let role_needs_update = file_contents(&fin_home.join("ROLE.md"))? != template_agents;
        let agents_needs_update = file_contents(&fin_home.join("AGENTS.md"))? != agents;
        let config_needs_update = match &desired_config {
            Some(config) => file_contents(&fin_home.join("fin.toml"))? != *config,
            None => false,
        };

        if role_needs_update {
            changed = true;
            println!("UPDATE fin {fin_slug}");
            println!("  ~ {}", fin_home.join("ROLE.md").display());
            if !dry_run {
                write_text(&fin_home.join("ROLE.md"), &template_agents)?;
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
        if let Some(desired_config) = desired_config.filter(|_| config_needs_update) {
            changed = true;
            if !role_needs_update && !agents_needs_update {
                println!("UPDATE fin {fin_slug}");
            }
            println!("  ~ {}", fin_home.join("fin.toml").display());
            if !dry_run {
                write_text(&fin_home.join("fin.toml"), &desired_config)?;
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

pub(super) fn sync_template_pod_agents(
    orqa: &Orqa,
    pod_root: &Path,
    template: &str,
    dry_run: bool,
) -> Result<bool, String> {
    let Some((source_path, agents)) = read_template_pod_agents(orqa, template)? else {
        return Ok(false);
    };

    let target_path = pod_root.join(".orqa").join("AGENTS.md");
    if file_contents(&target_path)? == agents {
        return Ok(false);
    }

    println!("UPDATE pod AGENTS.md from {}", source_path.display());
    println!("  ~ {}", target_path.display());
    if !dry_run {
        write_text(&target_path, &agents)?;
    }
    Ok(true)
}

fn read_template_pod_agents(
    orqa: &Orqa,
    template: &str,
) -> Result<Option<(PathBuf, String)>, String> {
    let template_dir = template_home(orqa, template);
    let Some(path) = template_pod_agents_path(&template_dir) else {
        return Ok(None);
    };
    let agents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(Some((path, agents)))
}

pub(super) fn create_fin_from_template(
    orqa: &Orqa,
    pod_slug: &str,
    pod_root: &Path,
    template: &str,
    fin_slug: &str,
    agents_source: &str,
    config: Option<&str>,
) -> Result<(), String> {
    let fin = FinRef::new(pod_slug, fin_slug)?;
    let fin_home = pod_root.join(".orqa").join("fins").join(fin_slug);
    let fin_config = materialized_fin_config(&fin, template, config)?;
    ensure_fin_runtime_homes(orqa, &fin)?;
    ensure_maildir(&fin_home.join("mail"))?;
    ensure_maildir(&fin_home.join("tasks"))?;

    write_if_missing(&fin_home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
    write_if_missing(&fin_home.join("fin.toml"), &fin_config)?;
    write_if_missing(&fin_home.join("ROLE.md"), agents_source)?;
    write_if_missing(
        &fin_home.join("AGENTS.md"),
        &fin_agents_template(&fin, agents_source),
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

fn materialized_fin_config(
    fin: &FinRef,
    template: &str,
    source_config: Option<&str>,
) -> Result<String, String> {
    let Some(source_config) = source_config else {
        return Ok(fin_config_template_with_backend_and_template(
            fin,
            None,
            Some(template),
        ));
    };

    let mut table = source_config.parse::<toml::Table>().map_err(|error| {
        format!(
            "failed to parse template fin config for {}: {error}",
            fin.fin
        )
    })?;
    stamp_fin_config(&mut table, template, &fin.fin)?;
    toml::to_string_pretty(&table).map_err(|error| {
        format!(
            "failed to serialize template fin config for {}: {error}",
            fin.fin
        )
    })
}

fn stamp_fin_config(table: &mut toml::Table, template: &str, fin: &str) -> Result<(), String> {
    let fin_value = table
        .entry("fin".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let fin_table = fin_value
        .as_table_mut()
        .ok_or_else(|| "template fin config [fin] must be a table".to_string())?;
    if let Some(existing_slug) = fin_table.get("slug") {
        let existing_slug = existing_slug
            .as_str()
            .ok_or_else(|| "template fin config [fin].slug must be a string".to_string())?;
        if existing_slug != fin {
            return Err(format!(
                "template fin config slug '{}' does not match fin '{}'",
                existing_slug, fin
            ));
        }
    }
    fin_table.insert("slug".to_string(), toml::Value::String(fin.to_string()));

    let mut template_table = toml::Table::new();
    template_table.insert(
        "name".to_string(),
        toml::Value::String(template.to_string()),
    );
    template_table.insert("fin".to_string(), toml::Value::String(fin.to_string()));
    table.insert("template".to_string(), toml::Value::Table(template_table));
    Ok(())
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

fn template_pod_agents_path(template_dir: &Path) -> Option<PathBuf> {
    let compact_style = template_dir.join("AGENTS.md");
    if compact_style.is_file() {
        return Some(compact_style);
    }

    let pod_style = template_dir.join(".orqa").join("AGENTS.md");
    if pod_style.is_file() {
        return Some(pod_style);
    }

    None
}

pub(super) struct TemplateFin {
    pub(super) slug: String,
    pub(super) agents_path: PathBuf,
    pub(super) config_path: Option<PathBuf>,
}

pub(super) fn template_fins(fins_dir: &Path) -> Result<Vec<TemplateFin>, String> {
    let mut fins = Vec::new();
    for slug in list_dirs(fins_dir)? {
        validate_slug(&slug)?;
        let fin_dir = fins_dir.join(&slug);
        let agents_path = template_fin_agents_path(&fin_dir);
        if !agents_path.is_file() {
            return Err(format!(
                "template fin '{}' is missing {} (or legacy ROLE.md)",
                slug,
                fin_dir.join("AGENTS.md").display()
            ));
        }
        let config_path = fin_dir.join("fin.toml");
        let config_path = if config_path.exists() {
            validate_template_fin_config(&config_path, &slug)?;
            Some(config_path)
        } else {
            None
        };
        fins.push(TemplateFin {
            slug,
            agents_path,
            config_path,
        });
    }
    Ok(fins)
}

fn template_fin_agents_path(fin_dir: &Path) -> PathBuf {
    let agents_path = fin_dir.join("AGENTS.md");
    if agents_path.is_file() {
        return agents_path;
    }
    fin_dir.join("ROLE.md")
}

fn read_template_fin_config(fin: &TemplateFin) -> Result<Option<String>, String> {
    fin.config_path
        .as_ref()
        .map(|path| {
            fs::read_to_string(path).map_err(|error| {
                format!(
                    "failed to read template fin config {}: {error}",
                    path.display()
                )
            })
        })
        .transpose()
}

fn validate_template_fin_config(path: &Path, fin: &str) -> Result<(), String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read template fin config {}: {error}",
            path.display()
        )
    })?;
    let parsed = contents.parse::<toml::Table>().map_err(|error| {
        format!(
            "failed to parse template fin config {}: {error}",
            path.display()
        )
    })?;
    if let Some(fin_table) = parsed.get("fin") {
        let fin_table = fin_table.as_table().ok_or_else(|| {
            format!(
                "template fin config {} [fin] must be a table",
                path.display()
            )
        })?;
        if let Some(slug) = fin_table.get("slug") {
            let slug = slug.as_str().ok_or_else(|| {
                format!(
                    "template fin config {} [fin].slug must be a string",
                    path.display()
                )
            })?;
            if slug != fin {
                return Err(format!(
                    "template fin config {} slug '{}' does not match fin '{}'",
                    path.display(),
                    slug,
                    fin
                ));
            }
        }
    }
    Ok(())
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

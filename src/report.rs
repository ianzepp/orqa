use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    cli::OpsReportArgs,
    mailbox::{field_value, message_id, sorted_files, split_front_matter},
    model::{FinRef, Orqa, PodRef},
    status::fin_status,
};

const CONTEXT_LIMIT: usize = 600;

pub(crate) fn ops_report(orqa: &Orqa, args: OpsReportArgs) -> Result<(), String> {
    let since = args.since.as_deref().map(parse_since).transpose()?;
    let pods = list_dirs(&orqa.home.join("pods"))?;

    println!("# Orqa Ops Report");
    println!();
    println!("- home: `{}`", orqa.home.display());
    println!(
        "- generated_at_unix: `{}`",
        unix_seconds(SystemTime::now())?
    );
    match &since {
        Some(since) => println!("- since: `{}`", since.label),
        None => println!("- since: `all`"),
    }
    println!("- pods: `{}`", pods.len());
    println!();

    print_operator_issues(orqa, since.as_ref())?;

    for pod in pods {
        print_pod(orqa, &pod, since.as_ref())?;
    }

    Ok(())
}

fn print_operator_issues(orqa: &Orqa, since: Option<&SinceFilter>) -> Result<(), String> {
    println!("## Operator Issues");
    let mut count = 0usize;
    for state in ["new", "cur", "closed"] {
        for path in sorted_files(&orqa.issues_home().join(state))? {
            if !include_path(&path, since)? {
                continue;
            }
            let contents = read_to_string(&path)?;
            let (fields, body) = split_front_matter(&contents);
            let id = message_id(&path)?;
            println!();
            println!("### `{}`", id);
            println!("- state: `{state}`");
            println!("- path: `{}`", path.display());
            for key in ["pod", "fin", "status", "severity", "kind", "title", "from"] {
                if let Some(value) = field_value(&fields, key) {
                    println!("- {key}: `{}`", inline(&value));
                }
            }
            print_context(body);
            count += 1;
        }
    }
    if count == 0 {
        println!();
        println!("No operator issues matched.");
    }
    println!();
    Ok(())
}

fn print_pod(orqa: &Orqa, pod: &str, since: Option<&SinceFilter>) -> Result<(), String> {
    let pod = PodRef::new(pod)?;
    let pod_home = orqa.pod_home(&pod);
    let fins = list_dirs(&pod_home.join("fins"))?;
    println!("## Pod `{}`", pod.slug);
    println!();
    println!("- path: `{}`", pod_home.display());
    println!("- fins: `{}`", fins.len());
    println!();

    for fin in fins {
        print_fin(orqa, &pod.slug, &fin, since)?;
    }

    Ok(())
}

fn print_fin(orqa: &Orqa, pod: &str, fin: &str, since: Option<&SinceFilter>) -> Result<(), String> {
    let fin = FinRef::new(pod, fin)?;
    let fin_home = orqa.fin_home(&fin);
    let status = fin_status(orqa, &fin)?;

    println!("### Fin `{}`", fin.fin);
    println!();
    println!("- path: `{}`", fin_home.display());
    println!("- sleeping: `{}`", status.sleeping);
    println!("- running: `{}`", status.running);
    println!("- unread_mail: `{}`", status.unread_mail);
    println!("- open_tasks: `{}`", status.open_tasks);
    if let Some(run) = status.last_run {
        println!(
            "- latest_run: `{}` status=`{}` path=`{}`",
            run.id,
            run.status,
            orqa.run_home(&fin, &run.id).display()
        );
    }
    println!();

    print_tasks(orqa, &fin, since)?;
    print_mail(orqa, &fin, since)?;
    Ok(())
}

fn print_tasks(orqa: &Orqa, fin: &FinRef, since: Option<&SinceFilter>) -> Result<(), String> {
    println!("#### Tasks");
    let mut count = 0usize;
    for state in ["new", "cur"] {
        for path in sorted_files(&orqa.task_home(fin).join(state))? {
            if !include_path(&path, since)? {
                continue;
            }
            let contents = read_to_string(&path)?;
            let (fields, body) = split_front_matter(&contents);
            let id = message_id(&path)?;
            println!(
                "- state=`{state}` status=`{}` priority=`{}` kind=`{}` from=`{}` to=`{}` title=`{}`",
                inline_field(&fields, "status"),
                inline_field(&fields, "priority"),
                inline_field(&fields, "kind"),
                inline_field(&fields, "from"),
                inline_field(&fields, "to"),
                inline_field(&fields, "title"),
            );
            print_compact_context(body);
            println!("  id=`{id}` path=`{}`", path.display());
            count += 1;
        }
    }
    if count == 0 {
        println!();
        println!("No tasks matched.");
    }
    println!();
    Ok(())
}

fn print_mail(orqa: &Orqa, fin: &FinRef, since: Option<&SinceFilter>) -> Result<(), String> {
    println!("#### Mail");
    let mut count = 0usize;
    for state in ["new", "cur"] {
        for path in sorted_files(&orqa.mail_home(fin).join(state))? {
            if !include_path(&path, since)? {
                continue;
            }
            let contents = read_to_string(&path)?;
            let (headers, body) = split_headers(&contents);
            let id = message_id(&path)?;
            println!(
                "- state=`{state}` from=`{}` to=`{}` subject=`{}`",
                inline(header_value(&headers, "From").unwrap_or("-")),
                inline(header_value(&headers, "To").unwrap_or("-")),
                inline(header_value(&headers, "Subject").unwrap_or("(no subject)")),
            );
            print_compact_context(body);
            println!("  id=`{id}` path=`{}`", path.display());
            count += 1;
        }
    }
    if count == 0 {
        println!();
        println!("No mail matched.");
    }
    println!();
    Ok(())
}

fn split_headers(message: &str) -> (Vec<(&str, &str)>, &str) {
    let Some((headers, body)) = message.split_once("\n\n") else {
        return (Vec::new(), message);
    };
    let headers = headers
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim(), value.trim()))
        })
        .collect();
    (headers, body)
}

fn header_value<'a>(headers: &'a [(&str, &str)], key: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header, _)| header.eq_ignore_ascii_case(key))
        .map(|(_, value)| *value)
}

fn print_context(body: &str) {
    let context = clip(body);
    if context.is_empty() {
        return;
    }
    println!();
    println!("```text");
    println!("{context}");
    println!("```");
}

fn print_compact_context(body: &str) {
    let context = clip(body);
    if context.is_empty() {
        return;
    }
    println!("  context=`{}`", inline(&context));
}

fn clip(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= CONTEXT_LIMIT {
        return trimmed.to_string();
    }

    let mut clipped = String::new();
    for character in trimmed.chars() {
        if clipped.len() + character.len_utf8() > CONTEXT_LIMIT {
            break;
        }
        clipped.push(character);
    }
    clipped.push_str("\n[clipped]");
    clipped
}

fn inline(value: &str) -> String {
    value.replace('`', "'").replace('\n', " ")
}

fn inline_field(fields: &[(String, String)], key: &str) -> String {
    field_value(fields, key)
        .map(|value| inline(&value))
        .unwrap_or_else(|| "-".to_string())
}

fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn list_dirs(dir: &Path) -> Result<Vec<String>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn include_path(path: &Path, since: Option<&SinceFilter>) -> Result<bool, String> {
    let Some(since) = since else {
        return Ok(true);
    };
    if let Some(time) = time_from_message_id(path) {
        return Ok(time >= since.time);
    }
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("failed to read metadata for {}: {error}", path.display()))?;
    Ok(modified >= since.time)
}

fn time_from_message_id(path: &Path) -> Option<SystemTime> {
    let micros = path
        .file_name()?
        .to_string_lossy()
        .split('.')
        .next()?
        .parse::<u64>()
        .ok()?;
    Some(UNIX_EPOCH + Duration::from_micros(micros))
}

struct SinceFilter {
    label: String,
    time: SystemTime,
}

fn parse_since(value: &str) -> Result<SinceFilter, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("--since cannot be empty".to_string());
    }
    if let Ok(seconds) = value.parse::<u64>() {
        return Ok(SinceFilter {
            label: value.to_string(),
            time: UNIX_EPOCH + Duration::from_secs(seconds),
        });
    }
    let duration = parse_duration(value)?;
    let now = SystemTime::now();
    let time = now
        .checked_sub(duration)
        .ok_or_else(|| format!("--since {value:?} is before the Unix epoch"))?;
    Ok(SinceFilter {
        label: format!("{value} ago"),
        time,
    })
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let split = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    if split == 0 {
        return Err(format!(
            "invalid --since {value:?}; use Unix seconds or a duration like 30m, 2h, or 1d"
        ));
    }
    let number = value[..split]
        .parse::<u64>()
        .map_err(|_| format!("invalid --since {value:?}; duration must start with a number"))?;
    let unit = value[split..].trim().to_ascii_lowercase();
    let seconds = match unit.as_str() {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => number,
        "m" | "min" | "mins" | "minute" | "minutes" => number * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => number * 60 * 60,
        "d" | "day" | "days" => number * 60 * 60 * 24,
        _ => {
            return Err(format!(
                "invalid --since {value:?}; use Unix seconds or a duration like 30m, 2h, or 1d"
            ));
        }
    };
    Ok(Duration::from_secs(seconds))
}

fn unix_seconds(time: SystemTime) -> Result<u64, String> {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))
}

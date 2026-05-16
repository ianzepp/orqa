use std::{env, ffi::OsString, fs};

use super::{claim_pidfile, parse_prompt_args, pidfile_matches};

#[test]
fn parses_prompt_args_json() {
    let args = parse_prompt_args(Some("[\"one\", \"two\"]")).ok();

    assert_eq!(
        args,
        Some(vec![OsString::from("one"), OsString::from("two")])
    );
}

#[test]
fn falls_back_to_empty_prompt_args_on_parse_error() {
    assert!(parse_prompt_args(Some("{bad json")).is_err());
}

#[test]
fn pidfile_matches_requires_exact_pid() {
    let root = env::temp_dir().join("orqa-loop-worker-test");
    assert!(fs::create_dir_all(&root).is_ok());

    let pid_path = root.join("pid");
    assert!(fs::write(&pid_path, "123\n").is_ok());

    assert!(pidfile_matches(&pid_path, 123));
    assert!(!pidfile_matches(&pid_path, 456));
    assert!(!pidfile_matches(&root.join("missing"), 123));

    assert!(fs::remove_dir_all(root).is_ok());
}

#[test]
fn claim_pidfile_writes_pid_before_first_wake() {
    let root = env::temp_dir().join("orqa-loop-worker-claim-test");
    let pid_path = root.join("nested").join("pid");
    let _ = fs::remove_dir_all(&root);

    assert!(claim_pidfile(&pid_path, 789).is_ok());
    assert_eq!(fs::read_to_string(&pid_path).ok().as_deref(), Some("789"));
    assert!(pidfile_matches(&pid_path, 789));

    assert!(fs::remove_dir_all(root).is_ok());
}

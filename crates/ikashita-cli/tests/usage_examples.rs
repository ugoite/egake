//! Offline acceptance tests for the checked-in usage projects.

use std::{path::PathBuf, process::Command};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ikashita"))
        .args(args)
        .output()
        .expect("ikashita CLI should run")
}

#[test]
fn read_only_csv_example_supports_offline_list_and_search() {
    let project = example("csv-readonly");
    let project = project.to_str().expect("example path is UTF-8");
    let output = run(&["list", project, "--resource", "catalog", "--query", "ada"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Ada Lovelace"));
    assert!(!stdout.contains("Grace Hopper"));
}

#[test]
fn multi_resource_example_validates_all_resources() {
    let project = example("multi-resource");
    let project = project.to_str().expect("example path is UTF-8");
    let output = run(&["test", project, "--json"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["ok"], true);
    let tests = value["tests"].as_array().expect("test list");
    assert!(tests.iter().any(|test| test["name"] == "resource:contacts"));
    assert!(tests.iter().any(|test| test["name"] == "resource:teams"));
}

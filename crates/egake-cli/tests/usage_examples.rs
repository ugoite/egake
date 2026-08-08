//! Offline acceptance tests for the checked-in usage projects.

use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_egake")).args(args).output().expect("egake CLI should run")
}

fn temporary_project() -> PathBuf {
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    std::env::temp_dir().join(format!("egake-cli-usage-{suffix}"))
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

#[test]
fn build_preserves_directory_output_and_can_emit_one_html_file() {
    let project = temporary_project();
    let project_string = project.to_str().expect("temporary path is UTF-8");
    let created = run(&["new", project_string]);
    assert!(created.status.success(), "{}", String::from_utf8_lossy(&created.stderr));

    let multi_default = run(&["build", project_string, "--output", "dist", "--json"]);
    assert!(multi_default.status.success(), "{}", String::from_utf8_lossy(&multi_default.stderr));
    assert_eq!(fs::read_dir(project.join("dist")).expect("initial dist").count(), 4);

    let single = run(&[
        "build",
        project_string,
        "--format",
        "single-html",
        "--output",
        "dist/site.html",
        "--json",
    ]);
    assert!(single.status.success(), "{}", String::from_utf8_lossy(&single.stderr));
    let single_result: serde_json::Value =
        serde_json::from_slice(&single.stdout).expect("single-html JSON output");
    assert_eq!(single_result["format"], "single-html");
    let single_path = project.join("dist/site.html");
    let html = fs::read_to_string(&single_path).expect("single HTML");
    assert_eq!(fs::read_dir(project.join("dist")).expect("dist").count(), 1);
    assert!(!html.contains("runtime.js"));
    assert!(!html.contains("runtime.css"));
    assert!(!html.contains("app.bundle.json"));
    assert!(html.contains("type=\"application/json\""));

    let shorthand = run(&["build", project_string, "--single-html", "--output", "alias"]);
    assert!(shorthand.status.success(), "{}", String::from_utf8_lossy(&shorthand.stderr));
    assert_eq!(fs::read_dir(project.join("alias")).expect("alias").count(), 1);

    let multi = run(&["build", project_string, "--output", "multi", "--json"]);
    assert!(multi.status.success(), "{}", String::from_utf8_lossy(&multi.stderr));
    let multi_dir = project.join("multi");
    let names = fs::read_dir(&multi_dir)
        .expect("multi output")
        .map(|entry| entry.expect("entry").file_name())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = ["index.html", "runtime.js", "runtime.css", "app.bundle.json"]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names, expected);

    fs::remove_dir_all(project).expect("cleanup");
}

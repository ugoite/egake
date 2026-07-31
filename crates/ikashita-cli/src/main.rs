//! The `ikashita` project CLI and local static runtime host.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    net::{IpAddr, SocketAddr},
    path::{Component as PathComponent, Path, PathBuf},
    process,
    sync::Arc,
};

use clap::{Args, Parser, Subcommand};
use ikashita_csv::{CsvResourceConfig, CsvResourceProvider};
use ikashita_resource::{Capability, JsonResourceProvider, ListQuery, ResourceSchema};
use ikashita_server::{ServerConfig, ServerState, StaticBundle};
use ikashita_spec::{
    ActionDefinition, ActionStep, ActionStepKind, ApplicationDefinition, Component, Diagnostic,
    EventBinding, ResourceCapability,
};
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'none'; object-src 'none'; base-uri 'none'; form-action 'self'">
  <title>ikashita application</title>
  <link rel="stylesheet" href="runtime.css">
</head>
<body>
  <div id="ikashita-root" aria-live="polite"></div>
  <script src="runtime.js" defer></script>
</body>
</html>
"##;

const RUNTIME_JS: &str = include_str!("../assets/runtime.js");
const RUNTIME_CSS: &str = include_str!("../assets/runtime.css");

const PROJECT_ERROR_CODE: &str = "IK3000";
const RESOURCE_CONFIG_ERROR_CODE: &str = "IK3001";
const SCHEMA_ERROR_CODE: &str = "IK3002";
const DATA_ERROR_CODE: &str = "IK3003";

#[derive(Debug, Parser)]
#[command(name = "ikashita", version, about = "Build and run KDL application projects")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a working starter application project.
    New(NewArgs),
    /// Parse, validate, and check the project's application schemas and data.
    Validate(ProjectArgs),
    /// Print a stable summary of the validated application.
    Inspect(ProjectArgs),
    /// Emit a self-contained static application bundle.
    Build(BuildArgs),
    /// Serve the application and its configured CSV resources.
    Run(ServeArgs),
    /// Serve the application in development mode.
    Dev(ServeArgs),
    /// Run deterministic project validation checks.
    Test(ProjectArgs),
}

#[derive(Debug, Args)]
struct ProjectArgs {
    /// Project directory containing ikashita.toml.
    #[arg(short, long, value_name = "DIR")]
    project: Option<PathBuf>,
    /// Project directory positional shorthand.
    #[arg(default_value = ".", value_name = "DIR")]
    path: PathBuf,
    /// Emit a machine-readable result.
    #[arg(long)]
    json: bool,
}

impl ProjectArgs {
    fn directory(&self) -> &Path {
        self.project.as_deref().unwrap_or(&self.path)
    }
}

#[derive(Debug, Args)]
struct BuildArgs {
    #[command(flatten)]
    project: ProjectArgs,
    /// Output directory relative to the project directory.
    #[arg(short, long, default_value = "dist", value_name = "DIR")]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Project directory containing ikashita.toml.
    #[arg(short, long, value_name = "DIR")]
    project: Option<PathBuf>,
    /// Project directory positional shorthand.
    #[arg(default_value = ".", value_name = "DIR")]
    path: PathBuf,
    /// IP address to bind. Non-loopback addresses require --allow-external.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// TCP port to bind.
    #[arg(long, default_value_t = 8787)]
    port: u16,
    /// Explicitly allow a non-loopback listen address.
    #[arg(long)]
    allow_external: bool,
}

impl ServeArgs {
    fn directory(&self) -> &Path {
        self.project.as_deref().unwrap_or(&self.path)
    }
}

#[derive(Debug, Args)]
struct NewArgs {
    /// Directory to create.
    #[arg(default_value = ".", value_name = "DIR")]
    path: PathBuf,
    /// Application name used in the generated KDL definition.
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    /// Emit a machine-readable result.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectConfig {
    app: RawAppConfig,
    #[serde(default)]
    resources: BTreeMap<String, RawResourceConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAppConfig {
    name: Option<String>,
    definition: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResourceConfig {
    path: Option<PathBuf>,
    #[serde(default = "default_resource_key")]
    key: String,
    #[serde(default)]
    writable: bool,
    #[serde(default)]
    backup_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResourceConfig {
    path: PathBuf,
    key: String,
    writable: bool,
    backup_count: u8,
}

#[derive(Clone, Debug)]
struct Project {
    root: PathBuf,
    definition_path: PathBuf,
    definition_relative: PathBuf,
    resources: BTreeMap<String, ResourceConfig>,
    configured_app_name: Option<String>,
}

#[derive(Clone, Debug)]
struct ValidatedProject {
    project: Project,
    definition: ApplicationDefinition,
}

#[derive(Clone, Debug)]
struct SchemaInfo {
    required: BTreeSet<String>,
    properties: BTreeMap<String, PropertyInfo>,
}

#[derive(Clone, Debug)]
struct PropertyInfo {
    type_name: Option<String>,
    enum_values: Option<Vec<Value>>,
    format: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct OutputLocation {
    file: Option<String>,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Serialize)]
struct CliDiagnostic {
    severity: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<OutputLocation>,
}

impl fmt::Display for CliDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}: ", self.severity, self.code)?;
        if let Some(location) = &self.location {
            write!(
                formatter,
                "{}:{}:{}: ",
                location.file.as_deref().unwrap_or("<input>"),
                location.line,
                location.column
            )?;
        }
        formatter.write_str(&self.message)
    }
}

#[derive(Clone, Debug)]
struct CliError {
    message: Option<String>,
    diagnostics: Vec<CliDiagnostic>,
    json: bool,
}

impl CliError {
    fn message(message: impl Into<String>) -> Self {
        Self { message: Some(message.into()), diagnostics: Vec::new(), json: false }
    }

    fn diagnostic(diagnostic: CliDiagnostic) -> Self {
        Self { message: None, diagnostics: vec![diagnostic], json: false }
    }

    fn diagnostics(diagnostics: Vec<CliDiagnostic>) -> Self {
        Self { message: None, diagnostics, json: false }
    }

    fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }
}

fn main() {
    let exit_code = match execute(Cli::parse()) {
        Ok(()) => 0,
        Err(error) => {
            report_error(&error);
            1
        }
    };
    process::exit(exit_code);
}

fn execute(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::New(args) => command_new(args),
        Command::Validate(args) => command_validate(args),
        Command::Inspect(args) => command_inspect(args),
        Command::Build(args) => command_build(args),
        Command::Run(args) => command_serve(args, false),
        Command::Dev(args) => command_serve(args, true),
        Command::Test(args) => command_test(args),
    }
}

fn command_new(args: NewArgs) -> Result<(), CliError> {
    let name = args
        .name
        .or_else(|| default_project_name(&args.path))
        .ok_or_else(|| CliError::message("application name must not be empty"))?;
    if name.trim().is_empty() || name.contains('"') || name.chars().any(char::is_control) {
        return Err(CliError::message(
            "application name must be non-empty and must not contain a quote",
        ))
        .map_err(|error| error.with_json(args.json));
    }
    scaffold_project(&args.path, &name).map_err(|error| error.with_json(args.json))?;
    if args.json {
        print_json(json!({ "ok": true, "project": args.path, "application": name }));
    } else {
        println!("created ikashita project {} ({name})", args.path.display());
    }
    Ok(())
}

fn command_validate(args: ProjectArgs) -> Result<(), CliError> {
    let validated = load_validated(args.directory(), args.json, true)?;
    if args.json {
        print_json(json!({
            "ok": true,
            "project": validated.project.root,
            "application": validated.definition.profile.name,
            "diagnostics": Vec::<Value>::new()
        }));
    } else {
        println!(
            "valid {} ({})",
            validated.project.root.display(),
            validated.definition.profile.name
        );
    }
    Ok(())
}

fn command_inspect(args: ProjectArgs) -> Result<(), CliError> {
    let validated = load_validated(args.directory(), args.json, true)?;
    let value = definition_to_value(&validated.definition);
    if args.json {
        print_json(json!({ "ok": true, "project": validated.project.root, "application": value }));
    } else {
        print_inspect(&validated.project, &validated.definition);
    }
    Ok(())
}

fn command_build(args: BuildArgs) -> Result<(), CliError> {
    let validated = load_validated(args.project.directory(), args.project.json, true)?;
    let output = safe_project_join(&validated.project.root, &args.output, "build output")
        .map_err(|error| error.with_json(args.project.json))?;
    let bundle =
        static_bundle(&validated.definition).map_err(|error| error.with_json(args.project.json))?;
    let files =
        write_bundle(&output, &bundle).map_err(|error| error.with_json(args.project.json))?;
    if args.project.json {
        print_json(json!({ "ok": true, "output": output, "files": files }));
    } else {
        println!("built {}", output.display());
        for file in files {
            println!("  {file}");
        }
    }
    Ok(())
}

fn command_test(args: ProjectArgs) -> Result<(), CliError> {
    let validated = load_validated(args.directory(), args.json, true)?;
    let mut checks = vec!["application-definition".to_owned(), "json-schemas".to_owned()];
    checks.extend(
        validated
            .definition
            .resources
            .iter()
            .filter(|resource| validated.project.resources.contains_key(&resource.name))
            .map(|resource| format!("resource:{}", resource.name)),
    );
    checks.push("static-bundle".to_owned());
    let _ = static_bundle(&validated.definition).map_err(|error| error.with_json(args.json))?;
    if args.json {
        let tests: Vec<Value> =
            checks.iter().map(|name| json!({ "name": name, "status": "ok" })).collect();
        print_json(json!({ "ok": true, "tests": tests }));
    } else {
        for check in &checks {
            println!("test {check} ... ok");
        }
        println!("{} project tests passed", checks.len());
    }
    Ok(())
}

fn command_serve(args: ServeArgs, dev: bool) -> Result<(), CliError> {
    let validated = load_validated(args.directory(), false, true)?;
    let bundle = static_bundle(&validated.definition)?;
    let host = args
        .host
        .parse::<IpAddr>()
        .map_err(|_| CliError::message("--host must be an IP address"))?;
    if !host.is_loopback() && !args.allow_external {
        return Err(CliError::message(format!(
            "refusing non-loopback listen address {host}; pass --allow-external to acknowledge that no authentication is enabled"
        )));
    }
    if !host.is_loopback() {
        eprintln!(
            "warning: listening on {host}:{} exposes the app externally; authentication is not enabled",
            args.port
        );
    }
    let state = Arc::new(ServerState::new().with_bundle(bundle));
    for resource in &validated.definition.resources {
        let config = validated.project.resources.get(&resource.name).ok_or_else(|| {
            CliError::message(format!(
                "resource '{}' has no resources.kdl/TOML configuration",
                resource.name
            ))
        })?;
        let csv_path = safe_project_join(&validated.project.root, &config.path, "CSV resource")?;
        let provider = CsvResourceProvider::open(
            CsvResourceConfig::new(csv_path)
                .with_name(resource.name.clone())
                .with_key(config.key.clone())
                .with_writable(config.writable)
                .with_backup_count(config.backup_count),
        )
        .map_err(|error| CliError::message(format!("resource '{}': {error}", resource.name)))?;
        state
            .register_provider(resource.name.clone(), provider)
            .map_err(|error| CliError::message(format!("resource '{}': {error}", resource.name)))?;
    }
    let address = SocketAddr::new(host, args.port);
    let config = ServerConfig::localhost().with_address(address);
    if dev {
        println!(
            "dev server listening on http://{address} (CORS disabled; static bundle in memory)"
        );
    } else {
        println!("server listening on http://{address} (CORS disabled)");
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| CliError::message("could not initialize the async server runtime"))?;
    runtime
        .block_on(ikashita_server::run(config, state))
        .map_err(|error| CliError::message(format!("server stopped: {error}")))
}

fn report_error(error: &CliError) {
    if error.json {
        print_json(
            json!({ "ok": false, "diagnostics": error.diagnostics, "message": error.message }),
        );
        return;
    }
    for diagnostic in &error.diagnostics {
        eprintln!("{diagnostic}");
    }
    if let Some(message) = &error.message {
        eprintln!("error {PROJECT_ERROR_CODE}: {message}");
    }
}

fn print_json(value: Value) {
    println!("{}", serde_json::to_string_pretty(&value).expect("JSON values are serializable"));
}

fn load_project(path: &Path) -> Result<Project, CliError> {
    let root = fs::canonicalize(path).map_err(|error| {
        CliError::message(format!("project directory '{}': {error}", path.display()))
    })?;
    if !root.is_dir() {
        return Err(CliError::message("project path must be a directory"));
    }
    let config_path = root.join("ikashita.toml");
    let config_source = fs::read_to_string(&config_path)
        .map_err(|error| CliError::message(format!("ikashita.toml: {error}")))?;
    let config: RawProjectConfig = toml::from_str(&config_source).map_err(|error| {
        CliError::message(format!("ikashita.toml: invalid configuration: {error}"))
    })?;
    let definition_relative = config.app.definition.unwrap_or_else(|| PathBuf::from("app.ui.kdl"));
    let definition_path = safe_project_join(&root, &definition_relative, "application definition")?;
    if !definition_path.is_file() {
        return Err(CliError::message(format!(
            "application definition '{}' is not a regular file",
            definition_relative.display()
        )));
    }

    let mut resources = BTreeMap::new();
    for (name, raw) in config.resources {
        let resource = raw_resource_config(raw)
            .map_err(|message| CliError::diagnostic(resource_config_diagnostic(&name, message)))?;
        if resources.insert(name.clone(), resource).is_some() {
            return Err(CliError::message(format!(
                "resource '{name}' is configured more than once"
            )));
        }
    }
    let resources_path = root.join("resources.kdl");
    if resources_path.exists() {
        let kdl_resources = parse_resources_kdl(&root, Path::new("resources.kdl"))?;
        if !resources.is_empty() {
            return Err(CliError::message(
                "resource configuration must use either resources.kdl or [resources.*] in ikashita.toml, not both",
            ));
        }
        resources = kdl_resources;
    }
    Ok(Project {
        root,
        definition_path,
        definition_relative,
        resources,
        configured_app_name: config.app.name,
    })
}

fn load_validated(
    path: &Path,
    json_output: bool,
    check_data: bool,
) -> Result<ValidatedProject, CliError> {
    let project = load_project(path).map_err(|error| error.with_json(json_output))?;
    let source = fs::read_to_string(&project.definition_path).map_err(|error| {
        CliError::message(format!("{}: {error}", project.definition_relative.display()))
            .with_json(json_output)
    })?;
    let definition = ApplicationDefinition::parse_and_validate_named(
        &source,
        project.definition_relative.display().to_string(),
    )
    .map_err(|diagnostics| {
        CliError::diagnostics(diagnostics.iter().map(spec_diagnostic).collect())
            .with_json(json_output)
    })?;
    let mut diagnostics = Vec::new();
    if let Some(configured_name) = &project.configured_app_name
        && configured_name != &definition.profile.name
    {
        diagnostics.push(project_diagnostic(
            PROJECT_ERROR_CODE,
            format!(
                "ikashita.toml app.name '{}' does not match app definition '{}'",
                configured_name, definition.profile.name
            ),
            Some("ikashita.toml"),
        ));
    }

    let declared: BTreeSet<&str> =
        definition.resources.iter().map(|resource| resource.name.as_str()).collect();
    for configured in project.resources.keys() {
        if !declared.contains(configured.as_str()) {
            diagnostics.push(project_diagnostic(
                RESOURCE_CONFIG_ERROR_CODE,
                format!("resource configuration '{configured}' is not declared in app.ui.kdl"),
                Some("resources.kdl"),
            ));
        }
    }

    for resource in &definition.resources {
        if !is_safe_resource_name(&resource.name) {
            diagnostics.push(project_diagnostic(
                RESOURCE_CONFIG_ERROR_CODE,
                format!("resource '{}' is not a safe API path segment", resource.name),
                Some(&project.definition_relative.display().to_string()),
            ));
        }
        let schema_path =
            safe_project_join(&project.root, Path::new(&resource.schema), "JSON schema")
                .map_err(|error| error.with_json(json_output))?;
        let schema_source = fs::read_to_string(&schema_path).map_err(|error| {
            CliError::diagnostic(project_diagnostic(
                SCHEMA_ERROR_CODE,
                format!("resource '{}' schema could not be read: {error}", resource.name),
                Some(&resource.schema),
            ))
            .with_json(json_output)
        })?;
        let schema: Value = serde_json::from_str(&schema_source).map_err(|error| {
            CliError::diagnostic(project_diagnostic(
                SCHEMA_ERROR_CODE,
                format!("resource '{}' schema is not valid JSON: {error}", resource.name),
                Some(&resource.schema),
            ))
            .with_json(json_output)
        })?;
        let info = validate_schema(&resource.name, &resource.schema, &schema, &mut diagnostics);
        let Some(config) = project.resources.get(&resource.name) else {
            diagnostics.push(project_diagnostic(
                RESOURCE_CONFIG_ERROR_CODE,
                format!("resource '{}' has no resource configuration", resource.name),
                Some("resources.kdl"),
            ));
            continue;
        };
        if check_data {
            match open_csv_provider(&project.root, &resource.name, config) {
                Ok(provider) => {
                    let provider_schema = provider.schema();
                    check_capabilities(resource, &provider_schema, &mut diagnostics);
                    if let (Ok(provider_schema), Some(info)) = (&provider_schema, &info) {
                        check_provider_schema(resource, provider_schema, info, &mut diagnostics);
                    }
                    if let Some(info) = &info {
                        validate_csv_data(
                            &resource.name,
                            &resource.schema,
                            &provider,
                            info,
                            &mut diagnostics,
                        );
                    }
                }
                Err(error) => diagnostics.push(project_diagnostic(
                    DATA_ERROR_CODE,
                    format!("resource '{}' could not be opened: {}", resource.name, error.message),
                    Some("resources.kdl"),
                )),
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(CliError::diagnostics(diagnostics).with_json(json_output));
    }
    Ok(ValidatedProject { project, definition })
}

fn open_csv_provider(
    root: &Path,
    name: &str,
    config: &ResourceConfig,
) -> Result<CsvResourceProvider, ikashita_resource::ResourceError> {
    let path = safe_project_join(root, &config.path, "CSV resource").map_err(|error| {
        ikashita_resource::ResourceError::new(
            ikashita_resource::ResourceErrorKind::Validation,
            error.message.unwrap_or_default(),
        )
    })?;
    CsvResourceProvider::open(
        CsvResourceConfig::new(path)
            .with_name(name)
            .with_key(config.key.clone())
            .with_writable(config.writable)
            .with_backup_count(config.backup_count),
    )
}

fn check_capabilities(
    resource: &ikashita_spec::ResourceDefinition,
    schema: &ikashita_resource::ResourceResult<ResourceSchema>,
    diagnostics: &mut Vec<CliDiagnostic>,
) {
    let Ok(schema) = schema else { return };
    for required in &resource.required_capabilities {
        let capability = match required {
            ResourceCapability::List => Capability::List,
            ResourceCapability::Get => Capability::Get,
            ResourceCapability::Create => Capability::Create,
            ResourceCapability::Update => Capability::Update,
            ResourceCapability::Delete => Capability::Delete,
            ResourceCapability::Invoke => Capability::Invoke,
        };
        if !schema.capabilities.contains(&capability) {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!(
                    "resource '{}' does not provide required capability '{required}'",
                    resource.name
                ),
                Some("resources.kdl"),
            ));
        }
    }
}

fn check_provider_schema(
    resource: &ikashita_spec::ResourceDefinition,
    provider: &ResourceSchema,
    external: &SchemaInfo,
    diagnostics: &mut Vec<CliDiagnostic>,
) {
    if provider.name != resource.name {
        diagnostics.push(project_diagnostic(
            DATA_ERROR_CODE,
            format!(
                "resource '{}' provider schema name does not match its declaration",
                resource.name
            ),
            Some("resources.kdl"),
        ));
    }
    let provider_fields: BTreeSet<&str> =
        provider.fields.iter().map(|field| field.name.as_str()).collect();
    for field in provider_fields.iter().copied() {
        if !external.properties.contains_key(field) {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!(
                    "resource '{}' provider field '{field}' is missing from its JSON schema",
                    resource.name
                ),
                Some("resources.kdl"),
            ));
        }
    }
    for (field, property) in &external.properties {
        if !provider_fields.contains(field.as_str()) {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!(
                    "resource '{}' JSON schema field '{field}' is missing from the CSV provider",
                    resource.name
                ),
                Some("resources.kdl"),
            ));
        } else if property.type_name.as_deref().is_some_and(|type_name| type_name != "string") {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!("resource '{}' CSV provider field '{field}' returns text but schema requires '{}'; use type=string", resource.name, property.type_name.as_deref().unwrap_or_default()),
                Some("resources.kdl"),
            ));
        }
    }
}

fn validate_csv_data(
    resource: &str,
    schema_file: &str,
    provider: &CsvResourceProvider,
    info: &SchemaInfo,
    diagnostics: &mut Vec<CliDiagnostic>,
) {
    let mut offset = 0_u64;
    let mut row_number = 0_usize;
    loop {
        let page = match provider.list(&ListQuery::new().with_pagination(offset, 500)) {
            Ok(page) => page,
            Err(error) => {
                diagnostics.push(project_diagnostic(
                    DATA_ERROR_CODE,
                    format!("resource '{resource}' data could not be checked: {error}"),
                    Some(schema_file),
                ));
                return;
            }
        };
        for value in &page.items {
            row_number += 1;
            validate_record(resource, schema_file, row_number, value, info, diagnostics);
        }
        if page.items.is_empty() || offset + page.items.len() as u64 >= page.total {
            break;
        }
        offset += page.items.len() as u64;
    }
}

fn validate_record(
    resource: &str,
    schema_file: &str,
    row_number: usize,
    value: &Value,
    info: &SchemaInfo,
    diagnostics: &mut Vec<CliDiagnostic>,
) {
    let Some(object) = value.as_object() else {
        diagnostics.push(project_diagnostic(
            DATA_ERROR_CODE,
            format!("resource '{resource}' row {row_number} must be a JSON object"),
            Some(schema_file),
        ));
        return;
    };
    for field in &info.required {
        if object.get(field).is_none_or(|value| value.is_null() || value.as_str() == Some("")) {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!(
                    "resource '{resource}' row {row_number} is missing required field '{field}'"
                ),
                Some(schema_file),
            ));
        }
    }
    for (field, property) in &info.properties {
        let Some(value) = object.get(field) else { continue };
        if let Some(type_name) = &property.type_name
            && !csv_type_matches(value, type_name)
        {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!("resource '{resource}' row {row_number} field '{field}' does not match schema type '{type_name}'"),
                Some(schema_file),
            ));
        }
        if let Some(enum_values) = &property.enum_values
            && !enum_values.iter().any(|expected| csv_enum_matches(value, expected))
        {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!("resource '{resource}' row {row_number} field '{field}' is not in the schema enum"),
                Some(schema_file),
            ));
        }
        if let Some(format) = &property.format
            && !csv_format_matches(value, format)
        {
            diagnostics.push(project_diagnostic(
                DATA_ERROR_CODE,
                format!("resource '{resource}' row {row_number} field '{field}' does not match format '{format}'"),
                Some(schema_file),
            ));
        }
    }
}

fn validate_schema(
    resource: &str,
    schema_file: &str,
    schema: &Value,
    diagnostics: &mut Vec<CliDiagnostic>,
) -> Option<SchemaInfo> {
    let Some(root) = schema.as_object() else {
        diagnostics.push(schema_diagnostic(
            schema_file,
            format!("resource '{resource}' schema must be a JSON object"),
        ));
        return None;
    };
    if root.get("type").and_then(Value::as_str) != Some("object") {
        diagnostics.push(schema_diagnostic(
            schema_file,
            format!("resource '{resource}' schema must declare type=object"),
        ));
    }
    let mut required = BTreeSet::new();
    if let Some(raw_required) = root.get("required") {
        match raw_required.as_array() {
            Some(items) => {
                for item in items {
                    if let Some(field) = item.as_str() {
                        if !required.insert(field.to_owned()) {
                            diagnostics.push(schema_diagnostic(
                                schema_file,
                                format!(
                                    "resource '{resource}' schema repeats required field '{field}'"
                                ),
                            ));
                        }
                    } else {
                        diagnostics.push(schema_diagnostic(
                            schema_file,
                            format!(
                                "resource '{resource}' schema required entries must be strings"
                            ),
                        ));
                    }
                }
            }
            None => diagnostics.push(schema_diagnostic(
                schema_file,
                format!("resource '{resource}' schema required must be an array"),
            )),
        }
    }
    let mut properties = BTreeMap::new();
    if let Some(raw_properties) = root.get("properties") {
        let Some(raw_properties) = raw_properties.as_object() else {
            diagnostics.push(schema_diagnostic(
                schema_file,
                format!("resource '{resource}' schema properties must be an object"),
            ));
            return Some(SchemaInfo { required, properties });
        };
        for (field, raw_property) in raw_properties {
            let Some(raw_property) = raw_property.as_object() else {
                diagnostics.push(schema_diagnostic(
                    schema_file,
                    format!("resource '{resource}' property '{field}' must be an object"),
                ));
                continue;
            };
            let type_name = match raw_property.get("type") {
                None => None,
                Some(value) => match value.as_str() {
                    Some(value) if valid_schema_type(value) => Some(value.to_owned()),
                    Some(value) => {
                        diagnostics.push(schema_diagnostic(schema_file, format!("resource '{resource}' property '{field}' has unsupported type '{value}'")));
                        None
                    }
                    None => {
                        diagnostics.push(schema_diagnostic(
                            schema_file,
                            format!(
                                "resource '{resource}' property '{field}' type must be a string"
                            ),
                        ));
                        None
                    }
                },
            };
            let enum_values = match raw_property.get("enum") {
                None => None,
                Some(value) => match value.as_array() {
                    Some(values) => Some(values.clone()),
                    None => {
                        diagnostics.push(schema_diagnostic(
                            schema_file,
                            format!(
                                "resource '{resource}' property '{field}' enum must be an array"
                            ),
                        ));
                        None
                    }
                },
            };
            let format = match raw_property.get("format") {
                None => None,
                Some(value) => {
                    match value.as_str() {
                        Some(value) if matches!(value, "email" | "date" | "date-time") => {
                            Some(value.to_owned())
                        }
                        Some(value) => {
                            diagnostics.push(schema_diagnostic(schema_file, format!("resource '{resource}' property '{field}' has unsupported format '{value}'")));
                            None
                        }
                        None => {
                            diagnostics.push(schema_diagnostic(schema_file, format!("resource '{resource}' property '{field}' format must be a string")));
                            None
                        }
                    }
                }
            };
            properties.insert(field.clone(), PropertyInfo { type_name, enum_values, format });
        }
    }
    for field in &required {
        if !properties.contains_key(field) {
            diagnostics.push(schema_diagnostic(
                schema_file,
                format!(
                    "resource '{resource}' required field '{field}' is not declared in properties"
                ),
            ));
        }
    }
    Some(SchemaInfo { required, properties })
}

fn valid_schema_type(value: &str) -> bool {
    matches!(value, "string" | "number" | "integer" | "boolean" | "object" | "array" | "null")
}

fn csv_type_matches(value: &Value, type_name: &str) -> bool {
    match type_name {
        "string" => value.is_string(),
        "number" => value.as_str().is_some_and(|value| value.parse::<f64>().is_ok()),
        "integer" => value.as_str().is_some_and(|value| value.parse::<i64>().is_ok()),
        "boolean" => value.as_str().is_some_and(|value| matches!(value, "true" | "false")),
        "null" => value.is_null(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => false,
    }
}

fn csv_enum_matches(value: &Value, expected: &Value) -> bool {
    match expected {
        Value::String(expected) => value.as_str() == Some(expected),
        Value::Number(expected) => {
            value.as_str().is_some_and(|value| value.parse::<f64>().ok() == expected.as_f64())
        }
        Value::Bool(expected) => {
            value.as_str().and_then(|value| value.parse::<bool>().ok()) == Some(*expected)
        }
        Value::Null => value.is_null(),
        _ => value == expected,
    }
}

fn csv_format_matches(value: &Value, format: &str) -> bool {
    let Some(value) = value.as_str() else { return false };
    match format {
        "email" => {
            let Some((local, domain)) = value.split_once('@') else { return false };
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !value.chars().any(char::is_whitespace)
        }
        "date" => {
            value.len() == 10
                && value.as_bytes()[4] == b'-'
                && value.as_bytes()[7] == b'-'
                && value
                    .chars()
                    .enumerate()
                    .all(|(index, char)| index == 4 || index == 7 || char.is_ascii_digit())
        }
        "date-time" => value.contains('T') && value.len() >= 12,
        _ => false,
    }
}

fn parse_resources_kdl(
    root: &Path,
    relative: &Path,
) -> Result<BTreeMap<String, ResourceConfig>, CliError> {
    let path = safe_project_join(root, relative, "resource configuration")?;
    let source = fs::read_to_string(&path)
        .map_err(|error| CliError::message(format!("{}: {error}", relative.display())))?;
    let document: KdlDocument = source.parse().map_err(|_| {
        CliError::message(format!("{}: invalid KDL resource configuration", relative.display()))
    })?;
    let mut nodes = Vec::new();
    for node in document.nodes() {
        match node.name().value() {
            "resources" => {
                let Some(children) = node.children() else {
                    return Err(CliError::message(
                        "resources.kdl: resources requires a children block",
                    ));
                };
                nodes.extend(children.nodes());
            }
            "csv" => nodes.push(node),
            unknown => {
                return Err(CliError::message(format!("resources.kdl: unknown node '{unknown}'")));
            }
        }
    }
    let mut resources = BTreeMap::new();
    for node in nodes {
        if node.name().value() != "csv" {
            return Err(CliError::message(format!(
                "resources.kdl: unsupported resource node '{}'",
                node.name().value()
            )));
        }
        let (name, config) = parse_csv_node(node)?;
        if resources.insert(name.clone(), config).is_some() {
            return Err(CliError::message(format!(
                "resources.kdl: resource '{name}' is declared more than once"
            )));
        }
    }
    Ok(resources)
}

fn parse_csv_node(node: &KdlNode) -> Result<(String, ResourceConfig), CliError> {
    let positional: Vec<&KdlEntry> =
        node.entries().iter().filter(|entry| entry.name().is_none()).collect();
    if positional.len() != 1 {
        return Err(CliError::message("resources.kdl: csv requires exactly one resource name"));
    }
    let name = positional[0]
        .value()
        .as_string()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            CliError::message("resources.kdl: csv resource name must be a non-empty string")
        })?
        .to_owned();
    if !is_safe_resource_name(&name) {
        return Err(CliError::message(
            "resources.kdl: csv resource name is not a safe API path segment",
        ));
    }
    let mut values = BTreeMap::new();
    for entry in node.entries().iter().filter(|entry| entry.name().is_some()) {
        let key = entry.name().expect("filtered property").value().to_owned();
        if !matches!(key.as_str(), "path" | "key" | "writable" | "backup-count") {
            return Err(CliError::message(format!("resources.kdl: unknown csv attribute '{key}'")));
        }
        if values.insert(key.clone(), entry.value().clone()).is_some() {
            return Err(CliError::message(format!(
                "resources.kdl: csv attribute '{key}' is repeated"
            )));
        }
    }
    let path = kdl_string(&values, "path")?
        .ok_or_else(|| CliError::message("resources.kdl: csv requires path=..."))?;
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| component == PathComponent::ParentDir)
    {
        return Err(CliError::message(
            "resources.kdl: csv path must be relative and must not contain '..'",
        ));
    }
    let key = kdl_string(&values, "key")?.unwrap_or_else(|| "id".to_owned());
    if key.trim().is_empty() {
        return Err(CliError::message("resources.kdl: csv key must not be empty"));
    }
    if key.chars().any(char::is_control) {
        return Err(CliError::message(
            "resources.kdl: csv key must not contain control characters",
        ));
    }
    let writable = kdl_bool(&values, "writable")?.unwrap_or(false);
    let backup_count = kdl_integer(&values, "backup-count")?.unwrap_or(0);
    let backup_count = u8::try_from(backup_count)
        .map_err(|_| CliError::message("resources.kdl: backup-count must be between 0 and 255"))?;
    Ok((name, ResourceConfig { path, key, writable, backup_count }))
}

fn kdl_string(values: &BTreeMap<String, KdlValue>, name: &str) -> Result<Option<String>, CliError> {
    let Some(value) = values.get(name) else { return Ok(None) };
    value
        .as_string()
        .map(str::to_owned)
        .ok_or_else(|| CliError::message(format!("resources.kdl: {name} must be a string")))
        .map(Some)
}

fn kdl_bool(values: &BTreeMap<String, KdlValue>, name: &str) -> Result<Option<bool>, CliError> {
    let Some(value) = values.get(name) else { return Ok(None) };
    value
        .as_bool()
        .ok_or_else(|| CliError::message(format!("resources.kdl: {name} must be a boolean")))
        .map(Some)
}

fn kdl_integer(values: &BTreeMap<String, KdlValue>, name: &str) -> Result<Option<i128>, CliError> {
    let Some(value) = values.get(name) else { return Ok(None) };
    value
        .as_integer()
        .ok_or_else(|| CliError::message(format!("resources.kdl: {name} must be an integer")))
        .map(Some)
}

fn raw_resource_config(raw: RawResourceConfig) -> Result<ResourceConfig, String> {
    let path = raw.path.ok_or_else(|| "resource requires path".to_owned())?;
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| component == PathComponent::ParentDir)
    {
        return Err("resource path must be relative and must not contain '..'".to_owned());
    }
    if raw.key.trim().is_empty() {
        return Err("resource key must not be empty".to_owned());
    }
    if raw.key.chars().any(char::is_control) {
        return Err("resource key must not contain control characters".to_owned());
    }
    Ok(ResourceConfig {
        path,
        key: raw.key,
        writable: raw.writable,
        backup_count: raw.backup_count,
    })
}

fn default_resource_key() -> String {
    "id".to_owned()
}

fn safe_project_join(root: &Path, relative: &Path, label: &str) -> Result<PathBuf, CliError> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(component, PathComponent::ParentDir | PathComponent::Prefix(_))
        })
    {
        return Err(CliError::message(format!(
            "{label} path must be relative and must not contain '..'"
        )));
    }
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        CliError::message(format!("{label} root could not be resolved safely: {error}"))
    })?;
    let candidate = root.join(relative);
    let mut existing = candidate.clone();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !existing.pop() {
                    return Err(CliError::message(format!(
                        "{label} path could not be resolved safely"
                    )));
                }
            }
            Err(error) => {
                return Err(CliError::message(format!(
                    "{label} path could not be resolved safely: {error}"
                )));
            }
        }
    }
    let canonical_existing = fs::canonicalize(&existing).map_err(|error| {
        CliError::message(format!("{label} path could not be resolved safely: {error}"))
    })?;
    if !canonical_existing.starts_with(&canonical_root) {
        return Err(CliError::message(format!(
            "{label} path resolves outside the project directory"
        )));
    }
    Ok(candidate)
}

fn is_safe_resource_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name != "."
        && name != ".."
        && !name.chars().any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn spec_diagnostic(diagnostic: &Diagnostic) -> CliDiagnostic {
    CliDiagnostic {
        severity: diagnostic.severity.to_string(),
        code: diagnostic.code.to_string(),
        message: diagnostic.message.clone(),
        location: diagnostic.location.as_ref().map(|location| OutputLocation {
            file: location.file.clone(),
            line: location.line,
            column: location.column,
        }),
    }
}

fn project_diagnostic(code: &str, message: impl Into<String>, file: Option<&str>) -> CliDiagnostic {
    CliDiagnostic {
        severity: "error".to_owned(),
        code: code.to_owned(),
        message: message.into(),
        location: file.map(|file| OutputLocation {
            file: Some(file.to_owned()),
            line: 1,
            column: 1,
        }),
    }
}

fn schema_diagnostic(file: &str, message: String) -> CliDiagnostic {
    project_diagnostic(SCHEMA_ERROR_CODE, message, Some(file))
}

fn resource_config_diagnostic(resource: &str, message: String) -> CliDiagnostic {
    project_diagnostic(
        RESOURCE_CONFIG_ERROR_CODE,
        format!("resource '{resource}': {message}"),
        Some("ikashita.toml"),
    )
}

fn static_bundle(definition: &ApplicationDefinition) -> Result<StaticBundle, CliError> {
    let application = serde_json::to_vec_pretty(&definition_to_value(definition))
        .map_err(|_| CliError::message("could not serialize the validated application bundle"))?;
    let mut bundle = StaticBundle::new(INDEX_HTML);
    bundle.insert_asset("runtime.js", RUNTIME_JS.as_bytes().to_vec());
    bundle.insert_asset("runtime.css", RUNTIME_CSS.as_bytes().to_vec());
    bundle.insert_asset("app.bundle.json", application);
    Ok(bundle)
}

fn write_bundle(output: &Path, bundle: &StaticBundle) -> Result<Vec<String>, CliError> {
    fs::create_dir_all(output).map_err(|error| {
        CliError::message(format!("could not create {}: {error}", output.display()))
    })?;
    fs::write(output.join("index.html"), bundle.index_html())
        .map_err(|error| CliError::message(format!("could not write index.html: {error}")))?;
    for (name, contents) in bundle.assets() {
        if name.is_empty()
            || name.starts_with('/')
            || name.contains('\\')
            || name.chars().any(char::is_control)
            || name
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return Err(CliError::message(format!("unsafe static asset path '{name}'")));
        }
        let path = safe_project_join(output, Path::new(name), "bundle asset")?;
        if name.contains('/')
            && let Some(parent) = path.parent()
        {
            fs::create_dir_all(parent).map_err(|error| {
                CliError::message(format!("could not create bundle asset directory: {error}"))
            })?;
        }
        fs::write(path, contents)
            .map_err(|error| CliError::message(format!("could not write {name}: {error}")))?;
    }
    Ok(std::iter::once("index.html".to_owned()).chain(bundle.assets().keys().cloned()).collect())
}

fn definition_to_value(definition: &ApplicationDefinition) -> Value {
    let mut resources = definition.resources.clone();
    resources.sort_by(|left, right| left.name.cmp(&right.name));
    let mut states = definition.states.clone();
    states.sort_by(|left, right| left.name.cmp(&right.name));
    let mut pages = definition.pages.clone();
    pages.sort_by(|left, right| left.name.cmp(&right.name));
    let mut actions = definition.actions.clone();
    actions.sort_by(|left, right| left.name.cmp(&right.name));
    json!({
        "profile": {
            "name": definition.profile.name,
            "version": definition.profile.version.to_string(),
        },
        "resources": resources.iter().map(|resource| json!({
            "name": resource.name,
            "schema": resource.schema,
            "capabilities": resource.required_capabilities.iter().map(|capability| capability.to_string()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "states": states.iter().map(|state| json!({ "name": state.name, "value": state.value })).collect::<Vec<_>>(),
        "pages": pages.iter().map(|page| json!({
            "name": page.name,
            "title": page.title,
            "components": page.components.iter().map(component_to_value).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "actions": actions.iter().map(action_to_value).collect::<Vec<_>>(),
    })
}

fn component_to_value(component: &Component) -> Value {
    json!({
        "kind": component.kind.as_str(),
        "id": component.id,
        "text": component.text,
        "attributes": component.attributes,
        "children": component.children.iter().map(component_to_value).collect::<Vec<_>>(),
        "events": component.events.iter().map(event_to_value).collect::<Vec<_>>(),
    })
}

fn event_to_value(event: &EventBinding) -> Value {
    json!({ "event": event.event, "action": event.action })
}

fn action_to_value(action: &ActionDefinition) -> Value {
    json!({
        "name": action.name,
        "steps": action.steps.iter().map(action_step_to_value).collect::<Vec<_>>(),
    })
}

fn action_step_to_value(step: &ActionStep) -> Value {
    json!({ "kind": action_step_kind_name(step.kind), "attributes": step.attributes, "text": step.text })
}

fn action_step_kind_name(kind: ActionStepKind) -> &'static str {
    match kind {
        ActionStepKind::Validate => "validate",
        ActionStepKind::Upsert => "upsert",
        ActionStepKind::Refresh => "refresh",
        ActionStepKind::Toast => "toast",
        ActionStepKind::Invoke => "invoke",
    }
}

fn print_inspect(project: &Project, definition: &ApplicationDefinition) {
    println!("project: {}", project.root.display());
    println!("application: {}", definition.profile.name);
    println!("profile: {}", definition.profile.version);
    println!("resources:");
    let mut resources = definition.resources.clone();
    resources.sort_by(|left, right| left.name.cmp(&right.name));
    for resource in resources {
        let capabilities = resource
            .required_capabilities
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        println!("  {} schema={} require={capabilities}", resource.name, resource.schema);
    }
    println!("pages:");
    let mut pages = definition.pages.clone();
    pages.sort_by(|left, right| left.name.cmp(&right.name));
    for page in pages {
        let component_count = page.components.iter().map(component_count).sum::<usize>();
        println!("  {} title={:?} components={component_count}", page.name, page.title);
    }
    println!("actions: {}", definition.actions.len());
}

fn component_count(component: &Component) -> usize {
    1 + component.children.iter().map(component_count).sum::<usize>()
}

fn default_project_name(path: &Path) -> Option<String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != ".")?;
    Some(name.to_owned())
}

fn scaffold_project(path: &Path, name: &str) -> Result<(), CliError> {
    if path.exists() {
        if !path.is_dir() {
            return Err(CliError::message(format!(
                "{} exists and is not a directory",
                path.display()
            )));
        }
        if fs::read_dir(path)
            .map_err(|error| {
                CliError::message(format!("could not inspect {}: {error}", path.display()))
            })?
            .next()
            .is_some()
        {
            return Err(CliError::message(format!(
                "refusing to scaffold into non-empty directory {}",
                path.display()
            )));
        }
    }
    fs::create_dir_all(path.join("schemas")).map_err(|error| {
        CliError::message(format!("could not create project directories: {error}"))
    })?;
    fs::create_dir_all(path.join("data")).map_err(|error| {
        CliError::message(format!("could not create project directories: {error}"))
    })?;
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    write_new_file(
        &path.join("ikashita.toml"),
        &format!("[app]\nname = \"{escaped}\"\ndefinition = \"app.ui.kdl\"\n"),
    )?;
    write_new_file(
        &path.join("app.ui.kdl"),
        &format!(
            "/- kdl-version 2\napp \"{escaped}\" version=\"0.1\" {{\n    resource \"contacts\" schema=\"schemas/contacts.schema.json\" {{\n        require \"list\"\n        require \"get\"\n        require \"create\"\n        require \"update\"\n        require \"delete\"\n    }}\n    state \"query\" value=\"\"\n    state \"draft\" value=#null\n    action \"open-create\"\n    action \"open-edit\"\n    action \"delete-contact\" {{ refresh resource=\"contacts\" }}\n    action \"save-contact\" {{\n        upsert resource=\"contacts\" value=\"$state.draft\"\n        refresh resource=\"contacts\"\n    }}\n    page \"main\" title=\"Contacts\" {{\n        column gap=\"md\" {{\n            row align=\"end\" {{\n                text-input label=\"Search\" bind=\"state.query\"\n                button \"Add\" action=\"open-create\" variant=\"primary\"\n            }}\n            data-table resource=\"contacts\" key=\"id\" {{\n                column field=\"name\" label=\"Name\"\n                column field=\"email\" label=\"Email\"\n                on \"select\" action=\"open-edit\"\n            }}\n            form id=\"editor\" bind=\"state.draft\" mode=\"drawer\" {{\n                text-input field=\"name\" label=\"Name\"\n                text-input field=\"email\" label=\"Email\"\n                textarea field=\"note\" label=\"Note\"\n                row {{\n                    button \"Delete\" variant=\"danger\" action=\"delete-contact\"\n                    button \"Save\" variant=\"primary\" action=\"save-contact\"\n                }}\n            }}\n        }}\n    }}\n}}\n"
        ),
    )?;
    write_new_file(
        &path.join("resources.kdl"),
        "/- kdl-version 2\nresources {\n    csv \"contacts\" path=\"data/contacts.csv\" key=\"id\" writable=#true backup-count=2\n}\n",
    )?;
    write_new_file(
        &path.join("schemas/contacts.schema.json"),
        "{\n  \"type\": \"object\",\n  \"required\": [\"id\", \"name\", \"email\"],\n  \"properties\": {\n    \"id\": { \"type\": \"string\" },\n    \"name\": { \"type\": \"string\" },\n    \"email\": { \"type\": \"string\", \"format\": \"email\" },\n    \"note\": { \"type\": \"string\" }\n  }\n}\n",
    )?;
    write_new_file(
        &path.join("data/contacts.csv"),
        "id,name,email,note\n1,Ada Lovelace,ada@example.com,Starter record\n",
    )?;
    write_new_file(
        &path.join("actions.rhai"),
        "# Placeholder for a future declarative action host.\n# The ikashita CLI never executes this file in the MVP.\n",
    )?;
    write_new_file(
        &path.join("README.md"),
        "# ikashita application\n\nRun `ikashita validate`, then `ikashita build` or `ikashita run`.\n\n`actions.rhai` is documentation-only in this MVP and is never executed by the CLI.\n",
    )?;
    Ok(())
}

fn write_new_file(path: &Path, contents: &str) -> Result<(), CliError> {
    fs::write(path, contents)
        .map_err(|error| CliError::message(format!("could not write {}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_project() -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let sequence = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("ikashita-cli-test-{}-{suffix}-{sequence}", process::id()))
    }

    #[test]
    fn scaffold_is_valid_and_builds_a_self_contained_bundle() {
        let path = temp_project();
        scaffold_project(&path, "test-app").expect("scaffold");
        let validated = load_validated(&path, false, true).expect("validation");
        let bundle = static_bundle(&validated.definition).expect("bundle");
        assert!(bundle.index_html().contains("Content-Security-Policy"));
        assert!(bundle.assets().contains_key("app.bundle.json"));
        let runtime = String::from_utf8_lossy(bundle.assets().get("runtime.js").expect("runtime"));
        assert!(!runtime.contains("eval("));
        assert!(!runtime.contains("innerHTML"));
        assert!(runtime.contains("/api/ikashita/v1"));
        assert!(runtime.contains("method: \"PATCH\""));
        assert!(runtime.contains("window.confirm"));
        assert!(runtime.contains("request_id"));
        fs::remove_dir_all(path).expect("cleanup");
    }

    #[test]
    fn schema_validation_rejects_invalid_email_without_printing_the_value() {
        let path = temp_project();
        scaffold_project(&path, "test-app").expect("scaffold");
        fs::write(path.join("data/contacts.csv"), "id,name,email,note\n1,Ada,not-an-email,Note\n")
            .expect("data");
        let error = load_validated(&path, false, true).expect_err("invalid data");
        let rendered =
            error.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains("format 'email'"));
        assert!(!rendered.contains("not-an-email"));
        fs::remove_dir_all(path).expect("cleanup");
    }

    #[test]
    fn schema_and_csv_provider_type_mismatches_are_diagnostics() {
        let path = temp_project();
        scaffold_project(&path, "test-app").expect("scaffold");
        let schema_path = path.join("schemas/contacts.schema.json");
        let schema = fs::read_to_string(&schema_path)
            .expect("schema")
            .replace("\"name\": { \"type\": \"string\" }", "\"name\": { \"type\": \"number\" }");
        fs::write(schema_path, schema).expect("schema");
        let error = load_validated(&path, false, true).expect_err("provider mismatch");
        let rendered =
            error.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains("returns text"));
        fs::remove_dir_all(path).expect("cleanup");
    }

    #[test]
    fn resources_kdl_is_deterministic_and_rejects_traversal() {
        let path = temp_project();
        fs::create_dir_all(&path).expect("project");
        fs::write(
            path.join("resources.kdl"),
            "/- kdl-version 2\nresources { csv \"contacts\" path=\"data/contacts.csv\" }\n",
        )
        .expect("resources");
        let resources = parse_resources_kdl(&path, Path::new("resources.kdl")).expect("parse");
        assert_eq!(resources["contacts"].key, "id");
        fs::write(
            path.join("resources.kdl"),
            "/- kdl-version 2\nresources { csv \"contacts\" path=\"../contacts.csv\" }\n",
        )
        .expect("resources");
        let error = parse_resources_kdl(&path, Path::new("resources.kdl")).expect_err("traversal");
        assert!(error.message.expect("message").contains(".."));
        fs::remove_dir_all(path).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn project_joins_reject_symlink_escape() {
        use std::os::unix::fs::symlink;

        let project = temp_project();
        let outside = temp_project();
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(outside.join("secret.txt"), "secret").expect("outside file");
        symlink(&outside, project.join("link")).expect("symlink");

        let error = safe_project_join(&project, Path::new("link/secret.txt"), "test")
            .expect_err("symlink escape");
        assert!(error.message.expect("message").contains("outside"));
        fs::remove_dir_all(project).expect("cleanup");
        fs::remove_dir_all(outside).expect("cleanup");
    }
}

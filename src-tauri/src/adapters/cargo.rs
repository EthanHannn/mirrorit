use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use toml::Value;

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, ReadResult, SnapshotRef, ToolCapability, ToolContext,
    ToolDetection, ToolId,
};

const USER_PRIORITY: u32 = 100;
const PROJECT_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

trait CargoCommandRunner: Send + Sync {
    fn version(&self) -> Result<String, String>;
}

struct SystemCargoCommandRunner;

impl CargoCommandRunner for SystemCargoCommandRunner {
    fn version(&self) -> Result<String, String> {
        let output = Command::new("cargo")
            .arg("--version")
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

pub struct CargoAdapter {
    environment: BTreeMap<String, String>,
    cargo_home: Option<PathBuf>,
    command_runner: Arc<dyn CargoCommandRunner>,
}

impl CargoAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect::<BTreeMap<_, _>>();
        Self {
            cargo_home: cargo_home(&environment),
            environment,
            command_runner: Arc::new(SystemCargoCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_sources(
        cargo_home: Option<PathBuf>,
        environment: BTreeMap<String, String>,
        command_runner: Arc<dyn CargoCommandRunner>,
    ) -> Self {
        Self {
            environment,
            cargo_home,
            command_runner,
        }
    }

    fn read_config(
        &self,
        directory: &Path,
        scope: ConfigScope,
        priority: u32,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        let Some(path) = cargo_config_path(directory) else {
            return;
        };
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                diagnostics.push(format!("无法读取 {}：{error}", path.display()));
                return;
            }
        };
        let config = match content.parse::<Value>() {
            Ok(config) => config,
            Err(error) => {
                diagnostics.push(format!("无法解析 {}：{error}", path.display()));
                return;
            }
        };
        add_toml_sources(values, &config, scope, path.display().to_string(), priority);
    }
}

impl ConfigAdapter for CargoAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Cargo
    }

    fn detect(&self, _context: &DetectionContext) -> AdapterResult<ToolDetection> {
        let version = self
            .command_runner
            .version()
            .ok()
            .filter(|version| !version.is_empty());
        Ok(ToolDetection {
            tool: self.tool(),
            installed: version.is_some(),
            version,
            capabilities: vec![ToolCapability::Read],
        })
    }

    fn read(&self, context: &ToolContext) -> AdapterResult<ReadResult> {
        let mut values = BTreeMap::new();
        let mut diagnostics = Vec::new();

        if let Some(cargo_home) = &self.cargo_home {
            self.read_config(
                cargo_home,
                ConfigScope::User,
                USER_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        } else {
            diagnostics.push("未能定位 CARGO_HOME，跳过用户级 Cargo 配置。".into());
        }

        if context.include_project_sources {
            if let Some(directory) = &context.project_directory {
                self.read_config(
                    &Path::new(directory).join(".cargo"),
                    ConfigScope::Project,
                    PROJECT_PRIORITY,
                    &mut values,
                    &mut diagnostics,
                );
            }
        }

        add_environment_sources(&mut values, &self.environment);

        Ok(ReadResult {
            effective_config: EffectiveConfig {
                tool: self.tool(),
                values,
            },
            diagnostics,
        })
    }

    fn plan(&self, _request: PlanRequest<'_>) -> AdapterResult<ChangePlan> {
        Err(read_only_error())
    }

    fn apply(&self, _plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        Err(read_only_error())
    }

    fn rollback(&self, _snapshot: &SnapshotRef) -> AdapterResult<Operation> {
        Err(read_only_error())
    }

    fn health_check(&self, _target: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        Err(read_only_error())
    }
}

fn cargo_home(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("CARGO_HOME")
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get("USERPROFILE")
                .or_else(|| environment.get("HOME"))
                .filter(|value| !value.trim().is_empty())
                .map(|home| Path::new(home).join(".cargo"))
        })
}

fn cargo_config_path(directory: &Path) -> Option<PathBuf> {
    let toml_path = directory.join("config.toml");
    if toml_path.is_file() {
        return Some(toml_path);
    }
    let legacy_path = directory.join("config");
    legacy_path.is_file().then_some(legacy_path)
}

fn add_toml_sources(
    values: &mut BTreeMap<String, EffectiveValue>,
    config: &Value,
    scope: ConfigScope,
    location: String,
    priority: u32,
) {
    let Some(root) = config.as_table() else {
        return;
    };
    if let Some(source_table) = root.get("source").and_then(Value::as_table) {
        for (name, source) in source_table {
            let Some(source) = source.as_table() else {
                continue;
            };
            add_toml_string(
                values,
                source,
                "replace-with",
                format!("source.{name}.replace-with"),
                scope,
                &location,
                priority,
            );
            add_toml_string(
                values,
                source,
                "registry",
                format!("source.{name}.registry"),
                scope,
                &location,
                priority,
            );
        }
    }
    if let Some(registries) = root.get("registries").and_then(Value::as_table) {
        for (name, registry) in registries {
            let Some(registry) = registry.as_table() else {
                continue;
            };
            add_toml_string(
                values,
                registry,
                "index",
                format!("registries.{name}.index"),
                scope,
                &location,
                priority,
            );
        }
    }
    if let Some(registry) = root.get("registry").and_then(Value::as_table) {
        add_toml_string(
            values,
            registry,
            "default",
            "registry.default".into(),
            scope,
            &location,
            priority,
        );
    }
    if let Some(http) = root.get("http").and_then(Value::as_table) {
        add_toml_string(
            values,
            http,
            "proxy",
            "http.proxy".into(),
            scope,
            &location,
            priority,
        );
    }
}

fn add_toml_string(
    values: &mut BTreeMap<String, EffectiveValue>,
    table: &toml::map::Map<String, Value>,
    field: &str,
    key: String,
    scope: ConfigScope,
    location: &str,
    priority: u32,
) {
    if let Some(value) = table.get(field).and_then(Value::as_str) {
        add_source(
            values,
            key,
            value.to_owned(),
            scope,
            location.to_owned(),
            priority,
        );
    }
}

fn add_environment_sources(
    values: &mut BTreeMap<String, EffectiveValue>,
    environment: &BTreeMap<String, String>,
) {
    for (name, value) in environment {
        let key = match name.as_str() {
            "CARGO_HTTP_PROXY" => Some("http.proxy".into()),
            "CARGO_REGISTRY_DEFAULT" => Some("registry.default".into()),
            _ => cargo_environment_key(name),
        };
        if let Some(key) = key {
            add_source(
                values,
                key,
                value.clone(),
                ConfigScope::Environment,
                format!("环境变量 {name}"),
                ENVIRONMENT_PRIORITY,
            );
        }
    }
}

fn cargo_environment_key(name: &str) -> Option<String> {
    if let Some(value) = name
        .strip_prefix("CARGO_SOURCE_")
        .and_then(|value| value.strip_suffix("_REPLACE_WITH"))
    {
        return Some(format!(
            "source.{}.replace-with",
            normalize_environment_name(value)
        ));
    }
    if let Some(value) = name
        .strip_prefix("CARGO_SOURCE_")
        .and_then(|value| value.strip_suffix("_REGISTRY"))
    {
        return Some(format!(
            "source.{}.registry",
            normalize_environment_name(value)
        ));
    }
    name.strip_prefix("CARGO_REGISTRIES_")
        .and_then(|value| value.strip_suffix("_INDEX"))
        .map(|value| format!("registries.{}.index", normalize_environment_name(value)))
}

fn normalize_environment_name(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}

fn add_source(
    values: &mut BTreeMap<String, EffectiveValue>,
    key: String,
    value: String,
    scope: ConfigScope,
    location: String,
    priority: u32,
) {
    let redacted_value = redact_url_credentials(&value);
    let sensitive = redacted_value != value;
    let entry = values.entry(key).or_insert_with(|| EffectiveValue {
        value: None,
        sources: Vec::new(),
    });
    entry.sources.push(ConfigSource {
        scope,
        location,
        priority,
        sensitive,
        value: Some(redacted_value.clone()),
    });
    entry.value = Some(redacted_value);
}

fn redact_url_credentials(value: &str) -> String {
    let Some(scheme_end) = value.find("://") else {
        return value.into();
    };
    let authority_start = scheme_end + 3;
    let Some(at_offset) = value[authority_start..].find('@') else {
        return value.into();
    };
    let at = authority_start + at_offset;
    format!("{}***:***{}", &value[..authority_start], &value[at..])
}

fn read_only_error() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "Cargo 当前仅支持只读扫描；不会修改 Cargo 配置、凭据或缓存。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct FixtureRunner(Result<String, String>);

    impl CargoCommandRunner for FixtureRunner {
        fn version(&self) -> Result<String, String> {
            self.0.clone()
        }
    }

    fn adapter(cargo_home: Option<PathBuf>, environment: BTreeMap<String, String>) -> CargoAdapter {
        CargoAdapter::with_sources(
            cargo_home,
            environment,
            Arc::new(FixtureRunner(Ok("cargo 1.97.1".into()))),
        )
    }

    #[test]
    fn reads_user_source_replacement_and_named_registry() {
        let cargo_home = test_directory("user");
        fs::write(
            cargo_home.join("config.toml"),
            r#"
[source.crates-io]
replace-with = "mirror"

[source.mirror]
registry = "sparse+https://mirror.example/index/"

[registries.mirror]
index = "sparse+https://mirror.example/index/"
"#,
        )
        .expect("fixture config");
        let result = adapter(Some(cargo_home.clone()), BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(
            result.effective_config.values["source.crates-io.replace-with"]
                .value
                .as_deref(),
            Some("mirror")
        );
        assert_eq!(
            result.effective_config.values["registries.mirror.index"]
                .value
                .as_deref(),
            Some("sparse+https://mirror.example/index/")
        );

        fs::remove_dir_all(cargo_home).expect("fixture directory should be removed");
    }

    #[test]
    fn project_and_environment_sources_override_user_values() {
        let cargo_home = test_directory("precedence-home");
        let project = test_directory("precedence-project");
        fs::write(
            cargo_home.join("config.toml"),
            "[http]\nproxy = \"https://user.example\"",
        )
        .expect("user config");
        fs::create_dir_all(project.join(".cargo")).expect("project cargo directory");
        fs::write(
            project.join(".cargo/config.toml"),
            "[http]\nproxy = \"https://project.example\"",
        )
        .expect("project config");
        let result = adapter(
            Some(cargo_home.clone()),
            BTreeMap::from([("CARGO_HTTP_PROXY".into(), "https://env.example".into())]),
        )
        .read(&ToolContext {
            project_directory: Some(project.display().to_string()),
            include_project_sources: true,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["http.proxy"]
                .value
                .as_deref(),
            Some("https://env.example")
        );
        assert_eq!(
            result.effective_config.values["http.proxy"].sources.len(),
            3
        );

        fs::remove_dir_all(cargo_home).expect("fixture directory should be removed");
        fs::remove_dir_all(project).expect("fixture directory should be removed");
    }

    #[test]
    fn ignores_missing_configuration_without_diagnostic() {
        let cargo_home = test_directory("missing");
        let result = adapter(Some(cargo_home.clone()), BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert!(result.effective_config.values.is_empty());
        assert!(result.diagnostics.is_empty());

        fs::remove_dir_all(cargo_home).expect("fixture directory should be removed");
    }

    #[test]
    fn reports_invalid_toml_and_keeps_environment_values() {
        let cargo_home = test_directory("invalid");
        fs::write(cargo_home.join("config.toml"), "[source").expect("invalid config");
        let result = adapter(
            Some(cargo_home.clone()),
            BTreeMap::from([("CARGO_REGISTRY_DEFAULT".into(), "mirror".into())]),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["registry.default"]
                .value
                .as_deref(),
            Some("mirror")
        );
        assert_eq!(result.diagnostics.len(), 1);

        fs::remove_dir_all(cargo_home).expect("fixture directory should be removed");
    }

    #[test]
    fn redacts_url_credentials_before_exposing_values() {
        let cargo_home = test_directory("credentials");
        fs::write(
            cargo_home.join("config.toml"),
            "[http]\nproxy = \"https://alice:secret@proxy.example\"",
        )
        .expect("fixture config");
        let result = adapter(Some(cargo_home.clone()), BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        let value = &result.effective_config.values["http.proxy"];
        assert_eq!(
            value.value.as_deref(),
            Some("https://***:***@proxy.example")
        );
        assert!(value.sources[0].sensitive);

        fs::remove_dir_all(cargo_home).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-cargo-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde_json::Value;

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, ReadResult, SnapshotRef, ToolCapability, ToolContext,
    ToolDetection, ToolId,
};

const SYSTEM_PRIORITY: u32 = 100;
const USER_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

trait DockerCommandRunner: Send + Sync {
    fn version(&self) -> Result<String, String>;
}

struct SystemDockerCommandRunner;

impl DockerCommandRunner for SystemDockerCommandRunner {
    fn version(&self) -> Result<String, String> {
        let output = Command::new("docker")
            .arg("--version")
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

pub struct DockerAdapter {
    environment: BTreeMap<String, String>,
    docker_config_directory: Option<PathBuf>,
    system_daemon_path: Option<PathBuf>,
    command_runner: Arc<dyn DockerCommandRunner>,
}

impl DockerAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect::<BTreeMap<_, _>>();
        Self {
            docker_config_directory: docker_config_directory(&environment),
            system_daemon_path: system_daemon_path(&environment),
            environment,
            command_runner: Arc::new(SystemDockerCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_sources(
        docker_config_directory: Option<PathBuf>,
        system_daemon_path: Option<PathBuf>,
        environment: BTreeMap<String, String>,
        command_runner: Arc<dyn DockerCommandRunner>,
    ) -> Self {
        Self {
            docker_config_directory,
            system_daemon_path,
            environment,
            command_runner,
        }
    }

    fn read_json_file(
        &self,
        path: &Path,
        kind: ConfigFileKind,
        scope: ConfigScope,
        priority: u32,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                diagnostics.push(format!("无法读取 {}：{error}", path.display()));
                return;
            }
        };
        let config = match serde_json::from_str::<Value>(&content) {
            Ok(config) => config,
            Err(error) => {
                diagnostics.push(format!("无法解析 {}：{error}", path.display()));
                return;
            }
        };
        match kind {
            ConfigFileKind::Cli => {
                add_cli_proxy_sources(values, &config, scope, path.display().to_string(), priority)
            }
            ConfigFileKind::Daemon { windows } => add_daemon_sources(
                values,
                &config,
                scope,
                path.display().to_string(),
                priority,
                windows,
            ),
        }
    }
}

impl ConfigAdapter for DockerAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Docker
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

    fn read(&self, _context: &ToolContext) -> AdapterResult<ReadResult> {
        let mut values = BTreeMap::new();
        let mut diagnostics = Vec::new();

        if let Some(path) = &self.system_daemon_path {
            self.read_json_file(
                path,
                ConfigFileKind::Daemon { windows: false },
                ConfigScope::System,
                SYSTEM_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        }
        if let Some(directory) = &self.docker_config_directory {
            self.read_json_file(
                &directory.join("config.json"),
                ConfigFileKind::Cli,
                ConfigScope::User,
                USER_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
            self.read_json_file(
                &directory.join("daemon.json"),
                ConfigFileKind::Daemon { windows: false },
                ConfigScope::User,
                USER_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
            self.read_json_file(
                &directory.join("windows-daemon.json"),
                ConfigFileKind::Daemon { windows: true },
                ConfigScope::User,
                USER_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        } else {
            diagnostics.push("未能定位 DOCKER_CONFIG，跳过用户级 Docker 配置。".into());
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

enum ConfigFileKind {
    Cli,
    Daemon { windows: bool },
}

fn docker_config_directory(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("DOCKER_CONFIG")
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get("USERPROFILE")
                .or_else(|| environment.get("HOME"))
                .filter(|value| !value.trim().is_empty())
                .map(|home| Path::new(home).join(".docker"))
        })
}

fn system_daemon_path(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("PROGRAMDATA")
        .filter(|value| !value.trim().is_empty())
        .map(|program_data| Path::new(program_data).join("docker/config/daemon.json"))
}

fn add_cli_proxy_sources(
    values: &mut BTreeMap<String, EffectiveValue>,
    config: &Value,
    scope: ConfigScope,
    location: String,
    priority: u32,
) {
    let Some(proxy_contexts) = config.get("proxies").and_then(Value::as_object) else {
        return;
    };
    for (context, proxy) in proxy_contexts {
        let Some(proxy) = proxy.as_object() else {
            continue;
        };
        for (source_field, field) in [
            ("httpProxy", "http"),
            ("httpsProxy", "https"),
            ("noProxy", "no-proxy"),
        ] {
            if let Some(value) = proxy.get(source_field).and_then(Value::as_str) {
                add_source(
                    values,
                    format!("cli.proxy.{context}.{field}"),
                    value.into(),
                    scope,
                    location.clone(),
                    priority,
                );
            }
        }
    }
}

fn add_daemon_sources(
    values: &mut BTreeMap<String, EffectiveValue>,
    config: &Value,
    scope: ConfigScope,
    location: String,
    priority: u32,
    windows: bool,
) {
    let namespace = if windows {
        "daemon.windows"
    } else {
        "daemon.linux"
    };
    for (field, key_prefix) in [
        ("registry-mirrors", "registry-mirror"),
        ("insecure-registries", "insecure-registry"),
    ] {
        let Some(entries) = config.get(field).and_then(Value::as_array) else {
            continue;
        };
        for (index, value) in entries.iter().enumerate() {
            if let Some(value) = value.as_str() {
                add_source(
                    values,
                    format!("{namespace}.{key_prefix}.{index}"),
                    value.into(),
                    scope,
                    location.clone(),
                    priority,
                );
            }
        }
    }
}

fn add_environment_sources(
    values: &mut BTreeMap<String, EffectiveValue>,
    environment: &BTreeMap<String, String>,
) {
    for (name, field) in [
        ("HTTP_PROXY", "http"),
        ("HTTPS_PROXY", "https"),
        ("NO_PROXY", "no-proxy"),
    ] {
        if let Some(value) = environment.get(name) {
            add_source(
                values,
                format!("cli.proxy.default.{field}"),
                value.clone(),
                ConfigScope::Environment,
                format!("环境变量 {name}"),
                ENVIRONMENT_PRIORITY,
            );
        }
    }
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
        message: "Docker 当前仅支持只读扫描；不会修改守护进程、凭据或缓存。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct FixtureRunner(Result<String, String>);

    impl DockerCommandRunner for FixtureRunner {
        fn version(&self) -> Result<String, String> {
            self.0.clone()
        }
    }

    fn adapter(
        docker_config_directory: Option<PathBuf>,
        system_daemon_path: Option<PathBuf>,
        environment: BTreeMap<String, String>,
    ) -> DockerAdapter {
        DockerAdapter::with_sources(
            docker_config_directory,
            system_daemon_path,
            environment,
            Arc::new(FixtureRunner(Ok("Docker version 29.0.1".into()))),
        )
    }

    #[test]
    fn reads_user_daemon_registry_mirrors_without_reading_auth_configuration() {
        let directory = test_directory("user-daemon");
        fs::write(
            directory.join("config.json"),
            r#"{"auths":{"registry.example":{"auth":"secret"}},"proxies":{"default":{"httpsProxy":"https://proxy.example"}}}"#,
        )
        .expect("fixture CLI config");
        fs::write(
            directory.join("daemon.json"),
            r#"{"registry-mirrors":["https://mirror.example"],"insecure-registries":["registry.local"]}"#,
        )
        .expect("fixture daemon config");
        let result = adapter(Some(directory.clone()), None, BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(
            result.effective_config.values["daemon.linux.registry-mirror.0"]
                .value
                .as_deref(),
            Some("https://mirror.example")
        );
        assert_eq!(
            result.effective_config.values["cli.proxy.default.https"]
                .value
                .as_deref(),
            Some("https://proxy.example")
        );
        assert!(result
            .effective_config
            .values
            .keys()
            .all(|key| !key.contains("auth")));

        fs::remove_dir_all(directory).expect("fixture directory should be removed");
    }

    #[test]
    fn user_daemon_and_environment_override_lower_priority_sources() {
        let system_directory = test_directory("system");
        let user_directory = test_directory("user");
        let system_daemon = system_directory.join("daemon.json");
        fs::write(
            &system_daemon,
            r#"{"registry-mirrors":["https://system.example"]}"#,
        )
        .expect("system daemon config");
        fs::write(
            user_directory.join("daemon.json"),
            r#"{"registry-mirrors":["https://user.example"]}"#,
        )
        .expect("user daemon config");
        fs::write(
            user_directory.join("config.json"),
            r#"{"proxies":{"default":{"httpProxy":"https://user-proxy.example"}}}"#,
        )
        .expect("user CLI config");
        let result = adapter(
            Some(user_directory.clone()),
            Some(system_daemon),
            BTreeMap::from([("HTTP_PROXY".into(), "https://environment.example".into())]),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["daemon.linux.registry-mirror.0"]
                .value
                .as_deref(),
            Some("https://user.example")
        );
        assert_eq!(
            result.effective_config.values["cli.proxy.default.http"]
                .value
                .as_deref(),
            Some("https://environment.example")
        );

        fs::remove_dir_all(system_directory).expect("fixture directory should be removed");
        fs::remove_dir_all(user_directory).expect("fixture directory should be removed");
    }

    #[test]
    fn ignores_missing_configuration_without_diagnostic() {
        let directory = test_directory("missing");
        let result = adapter(Some(directory.clone()), None, BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert!(result.effective_config.values.is_empty());
        assert!(result.diagnostics.is_empty());

        fs::remove_dir_all(directory).expect("fixture directory should be removed");
    }

    #[test]
    fn reports_invalid_json_and_keeps_environment_values() {
        let directory = test_directory("invalid");
        fs::write(directory.join("daemon.json"), "{").expect("invalid daemon config");
        let result = adapter(
            Some(directory.clone()),
            None,
            BTreeMap::from([("NO_PROXY".into(), "localhost".into())]),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["cli.proxy.default.no-proxy"]
                .value
                .as_deref(),
            Some("localhost")
        );
        assert_eq!(result.diagnostics.len(), 1);

        fs::remove_dir_all(directory).expect("fixture directory should be removed");
    }

    #[test]
    fn redacts_credentials_before_exposing_proxy_values() {
        let directory = test_directory("credentials");
        fs::write(
            directory.join("config.json"),
            r#"{"proxies":{"default":{"httpProxy":"https://alice:secret@proxy.example"}}}"#,
        )
        .expect("fixture CLI config");
        let result = adapter(Some(directory.clone()), None, BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        let value = &result.effective_config.values["cli.proxy.default.http"];
        assert_eq!(
            value.value.as_deref(),
            Some("https://***:***@proxy.example")
        );
        assert!(value.sources[0].sensitive);

        fs::remove_dir_all(directory).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-docker-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

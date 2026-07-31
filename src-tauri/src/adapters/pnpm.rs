use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, ReadResult, SnapshotRef, ToolCapability, ToolContext,
    ToolDetection, ToolId,
};

const GLOBAL_PRIORITY: u32 = 50;
const USER_PRIORITY: u32 = 100;
const PROJECT_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

trait PnpmCommandRunner: Send + Sync {
    fn version(&self) -> Result<String, String>;
    fn global_config_path(&self) -> Result<String, String>;
}

struct SystemPnpmCommandRunner;

impl PnpmCommandRunner for SystemPnpmCommandRunner {
    fn version(&self) -> Result<String, String> {
        run_pnpm(&["--version"])
    }

    fn global_config_path(&self) -> Result<String, String> {
        run_pnpm(&["config", "get", "globalconfig"])
    }
}

fn run_pnpm(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("pnpm")
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub struct PnpmAdapter {
    environment: BTreeMap<String, String>,
    user_config_path: Option<PathBuf>,
    command_runner: Arc<dyn PnpmCommandRunner>,
}

impl PnpmAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect::<BTreeMap<_, _>>();
        Self {
            user_config_path: user_config_path(&environment),
            environment,
            command_runner: Arc::new(SystemPnpmCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_sources(
        user_config_path: Option<PathBuf>,
        environment: BTreeMap<String, String>,
        command_runner: Arc<dyn PnpmCommandRunner>,
    ) -> Self {
        Self {
            environment,
            user_config_path,
            command_runner,
        }
    }

    fn read_file(
        &self,
        path: &Path,
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
        let entries = parse_npmrc(&content, &path.display().to_string(), diagnostics);
        add_entries(values, entries, scope, path.display().to_string(), priority);
    }
}

impl ConfigAdapter for PnpmAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Pnpm
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

        match self.command_runner.global_config_path() {
            Ok(path) if !path.is_empty() && path != "undefined" => self.read_file(
                Path::new(&path),
                ConfigScope::System,
                GLOBAL_PRIORITY,
                &mut values,
                &mut diagnostics,
            ),
            Ok(_) => {}
            Err(_) => diagnostics.push("未能定位 pnpm 全局配置，已跳过该来源。".into()),
        }

        if let Some(path) = &self.user_config_path {
            self.read_file(
                path,
                ConfigScope::User,
                USER_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        } else {
            diagnostics.push("未能定位用户目录，跳过用户级 .npmrc。".into());
        }

        if context.include_project_sources {
            if let Some(directory) = &context.project_directory {
                self.read_file(
                    &Path::new(directory).join(".npmrc"),
                    ConfigScope::Project,
                    PROJECT_PRIORITY,
                    &mut values,
                    &mut diagnostics,
                );
            }
        }

        add_environment_entries(&mut values, &self.environment);

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

fn user_config_path(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("USERPROFILE")
        .or_else(|| environment.get("HOME"))
        .filter(|value| !value.trim().is_empty())
        .map(|directory| Path::new(directory).join(".npmrc"))
}

fn parse_npmrc(
    content: &str,
    location: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<(String, String)> {
    let mut entries = BTreeMap::new();
    for (line_number, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            diagnostics.push(format!(
                "{location} 第 {} 行格式无法识别。",
                line_number + 1
            ));
            continue;
        };
        let key = normalize_key(key);
        if is_supported_key(&key) && !is_sensitive_key(&key) {
            entries.insert(key, redact_credentials(value.trim()));
        }
    }
    entries.into_iter().collect()
}

fn add_environment_entries(
    values: &mut BTreeMap<String, EffectiveValue>,
    environment: &BTreeMap<String, String>,
) {
    let mut entries = BTreeMap::new();
    for (name, value) in environment {
        let lowercase_name = name.to_ascii_lowercase();
        let key = lowercase_name
            .strip_prefix("npm_config_")
            .or_else(|| lowercase_name.strip_prefix("pnpm_config_"));
        let Some(key) = key else {
            continue;
        };
        let key = normalize_key(key);
        if is_supported_key(&key) && !is_sensitive_key(&key) {
            entries.insert((key, name), redact_credentials(value));
        }
    }
    for ((key, name), value) in entries {
        add_entries(
            values,
            vec![(key, value)],
            ConfigScope::Environment,
            name.to_owned(),
            ENVIRONMENT_PRIORITY,
        );
    }
}

fn add_entries(
    values: &mut BTreeMap<String, EffectiveValue>,
    entries: Vec<(String, String)>,
    scope: ConfigScope,
    location: String,
    priority: u32,
) {
    for (key, value) in entries {
        let entry = values.entry(key).or_insert_with(|| EffectiveValue {
            value: None,
            sources: Vec::new(),
        });
        entry.sources.push(ConfigSource {
            scope,
            location: location.clone(),
            priority,
            sensitive: value.contains("://***@"),
            value: Some(value.clone()),
        });
        entry.value = Some(value);
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn is_supported_key(key: &str) -> bool {
    matches!(key, "registry" | "proxy" | "https-proxy") || key.ends_with(":registry")
}

fn is_sensitive_key(key: &str) -> bool {
    ["token", "password", "auth"]
        .iter()
        .any(|part| key.contains(part))
}

fn redact_credentials(value: &str) -> String {
    let Some(scheme_end) = value.find("://") else {
        return value.to_owned();
    };
    let credential_start = scheme_end + 3;
    let remainder = &value[credential_start..];
    let Some(credential_end) = remainder.find('@') else {
        return value.to_owned();
    };
    let authority_end = remainder
        .find('/')
        .map(|index| credential_start + index)
        .unwrap_or(value.len());
    let credential_end = credential_start + credential_end;
    if credential_end >= authority_end {
        return value.to_owned();
    }
    format!(
        "{}***@{}",
        &value[..credential_start],
        &value[credential_end + 1..]
    )
}

fn read_only_error() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "pnpm 当前仅支持只读扫描；不会修改 .npmrc、凭据或缓存。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct FixtureRunner {
        version: Result<String, String>,
        global_config_path: Result<String, String>,
    }

    impl PnpmCommandRunner for FixtureRunner {
        fn version(&self) -> Result<String, String> {
            self.version.clone()
        }

        fn global_config_path(&self) -> Result<String, String> {
            self.global_config_path.clone()
        }
    }

    fn adapter(
        user_config_path: Option<PathBuf>,
        environment: BTreeMap<String, String>,
        global_config_path: PathBuf,
    ) -> PnpmAdapter {
        PnpmAdapter::with_sources(
            user_config_path,
            environment,
            Arc::new(FixtureRunner {
                version: Ok("10.27.0".into()),
                global_config_path: Ok(global_config_path.display().to_string()),
            }),
        )
    }

    #[test]
    fn reads_global_user_project_and_environment_sources_in_order() {
        let root = test_directory("precedence");
        let global = root.join("global.rc");
        let user = root.join("user.npmrc");
        let project = root.join("project");
        fs::create_dir_all(&project).expect("project directory");
        fs::write(&global, "registry=https://global.example/\n").expect("global config");
        fs::write(&user, "registry=https://user.example/\n").expect("user config");
        fs::write(
            project.join(".npmrc"),
            "registry=https://project.example/\n",
        )
        .expect("project config");
        let result = adapter(
            Some(user),
            BTreeMap::from([(
                "PNPM_CONFIG_REGISTRY".into(),
                "https://environment.example/".into(),
            )]),
            global,
        )
        .read(&ToolContext {
            project_directory: Some(project.display().to_string()),
            include_project_sources: true,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["registry"].value.as_deref(),
            Some("https://environment.example/")
        );
        assert_eq!(result.effective_config.values["registry"].sources.len(), 4);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn ignores_missing_configuration_without_diagnostic() {
        let root = test_directory("missing");
        let result = adapter(
            Some(root.join("missing.npmrc")),
            BTreeMap::new(),
            root.join("rc"),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert!(result.effective_config.values.is_empty());
        assert!(result.diagnostics.is_empty());

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn reports_invalid_npmrc_and_keeps_environment_values() {
        let root = test_directory("invalid");
        let user = root.join("user.npmrc");
        fs::write(&user, "invalid\n").expect("user config");
        let result = adapter(
            Some(user),
            BTreeMap::from([(
                "NPM_CONFIG_HTTPS_PROXY".into(),
                "https://proxy.example/".into(),
            )]),
            root.join("rc"),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["https-proxy"]
                .value
                .as_deref(),
            Some("https://proxy.example/")
        );
        assert_eq!(result.diagnostics.len(), 1);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn excludes_auth_entries_and_redacts_url_credentials() {
        let root = test_directory("credentials");
        let user = root.join("user.npmrc");
        fs::write(
            &user,
            "//registry.example/:_authToken=secret\nregistry=https://alice:secret@registry.example/\n",
        )
        .expect("user config");
        let result = adapter(Some(user), BTreeMap::new(), root.join("rc"))
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(
            result.effective_config.values["registry"].value.as_deref(),
            Some("https://***@registry.example/")
        );
        assert!(result.effective_config.values["registry"].sources[0].sensitive);
        assert_eq!(result.effective_config.values.len(), 1);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-pnpm-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

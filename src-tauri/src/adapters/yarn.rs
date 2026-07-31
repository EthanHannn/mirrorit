use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde_yaml::Value;

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

trait YarnCommandRunner: Send + Sync {
    fn version(&self) -> Result<String, String>;
}

struct SystemYarnCommandRunner;

impl YarnCommandRunner for SystemYarnCommandRunner {
    fn version(&self) -> Result<String, String> {
        let output = Command::new("yarn")
            .arg("--version")
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

pub struct YarnAdapter {
    environment: BTreeMap<String, String>,
    user_directory: Option<PathBuf>,
    command_runner: Arc<dyn YarnCommandRunner>,
}

impl YarnAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect::<BTreeMap<_, _>>();
        Self {
            user_directory: user_directory(&environment),
            environment,
            command_runner: Arc::new(SystemYarnCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_sources(
        user_directory: Option<PathBuf>,
        environment: BTreeMap<String, String>,
        command_runner: Arc<dyn YarnCommandRunner>,
    ) -> Self {
        Self {
            environment,
            user_directory,
            command_runner,
        }
    }

    fn read_classic_file(
        &self,
        path: &Path,
        scope: ConfigScope,
        priority: u32,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        let content = match read_file(path, diagnostics) {
            Some(content) => content,
            None => return,
        };
        add_entries(
            values,
            parse_classic(&content, &path.display().to_string(), diagnostics),
            scope,
            path.display().to_string(),
            priority,
        );
    }

    fn read_berry_file(
        &self,
        path: &Path,
        scope: ConfigScope,
        priority: u32,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        let content = match read_file(path, diagnostics) {
            Some(content) => content,
            None => return,
        };
        add_entries(
            values,
            parse_berry(&content, &path.display().to_string(), diagnostics),
            scope,
            path.display().to_string(),
            priority,
        );
    }
}

impl ConfigAdapter for YarnAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Yarn
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
        let version = self.command_runner.version().ok();

        match version.as_deref().and_then(yarn_kind) {
            Some(YarnKind::Classic) => {
                self.read_tool_files(
                    context,
                    ".yarnrc",
                    YarnKind::Classic,
                    &mut values,
                    &mut diagnostics,
                );
            }
            Some(YarnKind::Berry) => {
                self.read_tool_files(
                    context,
                    ".yarnrc.yml",
                    YarnKind::Berry,
                    &mut values,
                    &mut diagnostics,
                );
            }
            None => diagnostics
                .push("未检测到可识别的 Yarn 版本，无法确定应读取 Classic 或 Berry 配置。".into()),
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

impl YarnAdapter {
    fn read_tool_files(
        &self,
        context: &ToolContext,
        filename: &str,
        kind: YarnKind,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        if let Some(directory) = &self.user_directory {
            self.read_config_file(
                &directory.join(filename),
                kind,
                ConfigScope::User,
                USER_PRIORITY,
                values,
                diagnostics,
            );
        } else {
            diagnostics.push("未能定位用户目录，跳过用户级 Yarn 配置。".into());
        }

        if context.include_project_sources {
            if let Some(directory) = &context.project_directory {
                self.read_config_file(
                    &Path::new(directory).join(filename),
                    kind,
                    ConfigScope::Project,
                    PROJECT_PRIORITY,
                    values,
                    diagnostics,
                );
            }
        }
    }

    fn read_config_file(
        &self,
        path: &Path,
        kind: YarnKind,
        scope: ConfigScope,
        priority: u32,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        match kind {
            YarnKind::Classic => self.read_classic_file(path, scope, priority, values, diagnostics),
            YarnKind::Berry => self.read_berry_file(path, scope, priority, values, diagnostics),
        }
    }
}

#[derive(Clone, Copy)]
enum YarnKind {
    Classic,
    Berry,
}

fn yarn_kind(version: &str) -> Option<YarnKind> {
    let major = version.trim().split('.').next()?.parse::<u32>().ok()?;
    if major <= 1 {
        Some(YarnKind::Classic)
    } else {
        Some(YarnKind::Berry)
    }
}

fn user_directory(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("USERPROFILE")
        .or_else(|| environment.get("HOME"))
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn read_file(path: &Path, diagnostics: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            diagnostics.push(format!("无法读取 {}：{error}", path.display()));
            None
        }
    }
}

fn parse_classic(
    content: &str,
    location: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<(String, String)> {
    let mut entries = BTreeMap::new();
    for (line_number, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once(char::is_whitespace) else {
            if is_supported_key(&normalize_key(line)) {
                diagnostics.push(format!(
                    "{location} 第 {} 行的 Yarn Classic 值格式无法识别。",
                    line_number + 1
                ));
            }
            continue;
        };
        let key = normalize_key(raw_key);
        if !is_supported_key(&key) {
            continue;
        }
        let Some(value) = unquote_classic_value(raw_value.trim()) else {
            diagnostics.push(format!(
                "{location} 第 {} 行的 Yarn Classic 值格式无法识别。",
                line_number + 1
            ));
            continue;
        };
        entries.insert(key, redact_credentials(value));
    }
    entries.into_iter().collect()
}

fn unquote_classic_value(value: &str) -> Option<&str> {
    let quote = value.chars().next()?;
    if !matches!(quote, '"' | '\'') || !value.ends_with(quote) || value.len() < 2 {
        return None;
    }
    Some(&value[1..value.len() - 1])
}

fn parse_berry(
    content: &str,
    location: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<(String, String)> {
    let document = match serde_yaml::from_str::<Value>(content) {
        Ok(document) => document,
        Err(error) => {
            diagnostics.push(format!("{location} YAML 无法解析：{error}"));
            return Vec::new();
        }
    };
    let Some(root) = document.as_mapping() else {
        diagnostics.push(format!("{location} 根节点不是 Yarn Berry 配置对象。"));
        return Vec::new();
    };

    let mut entries = BTreeMap::new();
    add_berry_value(root, "npmRegistryServer", "registry", &mut entries);
    add_berry_value(root, "httpProxy", "proxy", &mut entries);
    add_berry_value(root, "httpsProxy", "https-proxy", &mut entries);

    if let Some(scopes) = root
        .get(Value::String("npmScopes".into()))
        .and_then(Value::as_mapping)
    {
        for (scope, settings) in scopes {
            let Some(scope) = scope.as_str() else {
                continue;
            };
            let Some(settings) = settings.as_mapping() else {
                continue;
            };
            let key = format!("@{}:registry", scope.trim_start_matches('@'));
            add_berry_value(settings, "npmRegistryServer", &key, &mut entries);
        }
    }

    entries.into_iter().collect()
}

fn add_berry_value(
    mapping: &serde_yaml::Mapping,
    berry_key: &str,
    normalized_key: &str,
    entries: &mut BTreeMap<String, String>,
) {
    if let Some(value) = mapping
        .get(Value::String(berry_key.into()))
        .and_then(Value::as_str)
    {
        entries.insert(normalized_key.to_owned(), redact_credentials(value.trim()));
    }
}

fn add_environment_entries(
    values: &mut BTreeMap<String, EffectiveValue>,
    environment: &BTreeMap<String, String>,
) {
    for (name, key) in [
        ("YARN_NPM_REGISTRY_SERVER", "registry"),
        ("YARN_HTTP_PROXY", "proxy"),
        ("YARN_HTTPS_PROXY", "https-proxy"),
    ] {
        if let Some(value) = environment
            .get(name)
            .filter(|value| !value.trim().is_empty())
        {
            add_entries(
                values,
                vec![(key.into(), redact_credentials(value.trim()))],
                ConfigScope::Environment,
                name.into(),
                ENVIRONMENT_PRIORITY,
            );
        }
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
        message: "Yarn 当前仅支持只读扫描；不会修改配置、凭据或缓存。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct FixtureRunner(Result<String, String>);

    impl YarnCommandRunner for FixtureRunner {
        fn version(&self) -> Result<String, String> {
            self.0.clone()
        }
    }

    fn adapter(
        version: Result<&str, &str>,
        user_directory: Option<PathBuf>,
        environment: BTreeMap<String, String>,
    ) -> YarnAdapter {
        YarnAdapter::with_sources(
            user_directory,
            environment,
            Arc::new(FixtureRunner(
                version.map(str::to_owned).map_err(str::to_owned),
            )),
        )
    }

    #[test]
    fn classic_sources_follow_user_project_and_environment_precedence() {
        let root = test_directory("classic");
        let user = root.join("user");
        let project = root.join("project");
        fs::create_dir_all(&user).expect("user directory");
        fs::create_dir_all(&project).expect("project directory");
        fs::write(
            user.join(".yarnrc"),
            "registry \"https://user.example/\"\n@acme:registry \"https://scope.example/\"\n",
        )
        .expect("user config");
        fs::write(
            project.join(".yarnrc"),
            "registry \"https://project.example/\"\n",
        )
        .expect("project config");

        let result = adapter(
            Ok("1.22.22"),
            Some(user),
            BTreeMap::from([(
                "YARN_NPM_REGISTRY_SERVER".into(),
                "https://environment.example/".into(),
            )]),
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
        assert_eq!(result.effective_config.values["registry"].sources.len(), 3);
        assert_eq!(
            result.effective_config.values["@acme:registry"]
                .value
                .as_deref(),
            Some("https://scope.example/")
        );

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn berry_reads_registry_scopes_and_proxies() {
        let root = test_directory("berry");
        let user = root.join("user");
        fs::create_dir_all(&user).expect("user directory");
        fs::write(
            user.join(".yarnrc.yml"),
            "npmRegistryServer: https://registry.example/\nhttpProxy: http://proxy.example/\nhttpsProxy: https://secure-proxy.example/\nnpmScopes:\n  acme:\n    npmRegistryServer: https://acme.example/\n",
        )
        .expect("berry config");

        let result = adapter(Ok("4.5.0"), Some(user), BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(result.effective_config.values.len(), 4);
        assert_eq!(
            result.effective_config.values["@acme:registry"]
                .value
                .as_deref(),
            Some("https://acme.example/")
        );
        assert_eq!(
            result.effective_config.values["https-proxy"]
                .value
                .as_deref(),
            Some("https://secure-proxy.example/")
        );

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn missing_yarn_keeps_explicit_environment_values() {
        let result = adapter(
            Err("not found"),
            None,
            BTreeMap::from([("YARN_HTTP_PROXY".into(), "https://proxy.example/".into())]),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["proxy"].value.as_deref(),
            Some("https://proxy.example/")
        );
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn invalid_berry_config_keeps_environment_values() {
        let root = test_directory("invalid");
        let user = root.join("user");
        fs::create_dir_all(&user).expect("user directory");
        fs::write(user.join(".yarnrc.yml"), "npmRegistryServer: [").expect("berry config");

        let result = adapter(
            Ok("3.8.7"),
            Some(user),
            BTreeMap::from([("YARN_HTTPS_PROXY".into(), "https://proxy.example/".into())]),
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
    fn ignores_berry_auth_and_redacts_credentials() {
        let root = test_directory("credentials");
        let user = root.join("user");
        fs::create_dir_all(&user).expect("user directory");
        fs::write(
            user.join(".yarnrc.yml"),
            "npmAuthToken: secret\nnpmRegistryServer: https://alice:secret@registry.example/\n",
        )
        .expect("berry config");

        let result = adapter(Ok("4.0.0"), Some(user), BTreeMap::new())
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(result.effective_config.values.len(), 1);
        assert_eq!(
            result.effective_config.values["registry"].value.as_deref(),
            Some("https://***@registry.example/")
        );
        assert!(result.effective_config.values["registry"].sources[0].sensitive);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-yarn-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

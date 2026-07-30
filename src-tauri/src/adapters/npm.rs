use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, PlannedChange, Profile, ReadResult, SnapshotRef, ToolCapability,
    ToolContext, ToolDetection, ToolId,
};

const USER_CONFIG_PRIORITY: u32 = 100;
const PROJECT_CONFIG_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

#[derive(Debug, Clone)]
pub struct NpmAdapter {
    user_config_path: Option<PathBuf>,
    environment: BTreeMap<String, String>,
}

impl NpmAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect();
        let user_config_path = user_config_path(&environment);

        Self {
            user_config_path,
            environment,
        }
    }

    #[cfg(test)]
    fn with_sources(
        user_config_path: Option<PathBuf>,
        environment: BTreeMap<String, String>,
    ) -> Self {
        Self {
            user_config_path,
            environment,
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
        match fs::read_to_string(path) {
            Ok(content) => {
                let entries = parse_npmrc(&content, &path.display().to_string(), diagnostics);
                add_entries(values, entries, scope, path.display().to_string(), priority);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => diagnostics.push(format!("无法读取 {}：{error}", path.display())),
        }
    }
}

impl ConfigAdapter for NpmAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Npm
    }

    fn detect(&self, _context: &DetectionContext) -> AdapterResult<ToolDetection> {
        let version = Command::new("npm")
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        Ok(ToolDetection {
            tool: self.tool(),
            installed: version.is_some(),
            version,
            capabilities: vec![ToolCapability::Read, ToolCapability::Plan],
        })
    }

    fn read(&self, context: &ToolContext) -> AdapterResult<ReadResult> {
        let mut values = BTreeMap::new();
        let mut diagnostics = Vec::new();

        if let Some(path) = &self.user_config_path {
            self.read_file(
                path,
                ConfigScope::User,
                USER_CONFIG_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        } else {
            diagnostics.push("未能定位用户目录，跳过用户级 .npmrc。".into());
        }

        if context.include_project_sources {
            if let Some(directory) = &context.project_directory {
                let path = Path::new(directory).join(".npmrc");
                self.read_file(
                    &path,
                    ConfigScope::Project,
                    PROJECT_CONFIG_PRIORITY,
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

    fn plan(&self, request: PlanRequest<'_>) -> AdapterResult<ChangePlan> {
        let Some(path) = &self.user_config_path else {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "未能定位用户级 .npmrc，无法生成预览。".into(),
            });
        };

        self.plan_for_target(
            path,
            ConfigScope::User,
            USER_CONFIG_PRIORITY,
            request.profile,
            request.current_config,
        )
    }

    fn apply(&self, _plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        Err(unsupported_error("npm 配置写入将在后续阶段提供。"))
    }

    fn rollback(&self, _snapshot: &SnapshotRef) -> AdapterResult<Operation> {
        Err(unsupported_error("npm 配置回滚将在后续阶段提供。"))
    }

    fn health_check(&self, _target: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        Err(unsupported_error("npm 连通性检查将在后续阶段提供。"))
    }
}

impl NpmAdapter {
    pub fn plan_for_target(
        &self,
        path: &Path,
        scope: ConfigScope,
        priority: u32,
        profile: &Profile,
        current_config: &ReadResult,
    ) -> AdapterResult<ChangePlan> {
        let file = path.display().to_string();
        let file_checksum = file_checksum(path)?;
        let changes = profile
            .values
            .iter()
            .filter(|(field, _)| is_supported_key(field))
            .map(|(field, next_value)| {
                let effective_value = current_config.effective_config.values.get(field);
                let previous_value = effective_value.and_then(|value| {
                    value
                        .sources
                        .iter()
                        .rev()
                        .find(|source| source.scope == scope && source.location == file)
                        .and_then(|source| source.value.clone())
                });
                let overridden = effective_value
                    .and_then(|value| value.sources.last())
                    .is_some_and(|source| source.priority > priority);

                PlannedChange {
                    file: file.clone(),
                    field: field.clone(),
                    previous_value,
                    next_value: Some(next_value.as_str().to_owned()),
                    risk: overridden
                        .then(|| "存在更高优先级的环境变量，应用后该值可能不会生效。".into()),
                }
            })
            .collect();

        Ok(ChangePlan {
            id: format!("npm-{}-{}", profile.id, scope_label(scope)),
            tool: self.tool(),
            file_checksums: BTreeMap::from([(file, file_checksum)]),
            changes,
        })
    }
}

fn user_config_path(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("USERPROFILE")
        .or_else(|| environment.get("HOME"))
        .map(|directory| Path::new(directory).join(".npmrc"))
}

fn parse_npmrc(
    content: &str,
    location: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<(String, String)> {
    let mut entries = BTreeMap::new();

    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            diagnostics.push(format!("{} 第 {} 行格式无法识别。", location, index + 1));
            continue;
        };

        let key = normalize_key(key);
        if !is_supported_key(&key) {
            continue;
        }

        if is_sensitive_key(&key) {
            diagnostics.push(format!(
                "{} 第 {} 行包含敏感配置，已忽略。",
                location,
                index + 1
            ));
            continue;
        }

        entries.insert(key, redact_credentials(value.trim()));
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
        let Some(key) = lowercase_name.strip_prefix("npm_config_") else {
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
        let source = ConfigSource {
            scope,
            location: location.clone(),
            priority,
            sensitive: value.contains("://***@"),
            value: Some(value.clone()),
        };
        let entry = values.entry(key).or_insert_with(|| EffectiveValue {
            value: None,
            sources: Vec::new(),
        });

        entry.value = Some(value);
        entry.sources.push(source);
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

fn unsupported_error(message: &str) -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: message.into(),
    }
}

fn file_checksum(path: &Path) -> AdapterResult<String> {
    match fs::read(path) {
        Ok(content) => Ok(format!("{:x}", Sha256::digest(content))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("missing".into()),
        Err(error) => Err(AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 {}：{error}", path.display()),
        }),
    }
}

fn scope_label(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::System => "system",
        ConfigScope::User => "user",
        ConfigScope::Project => "project",
        ConfigScope::Environment => "environment",
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{NonSensitiveValue, Profile};

    use super::*;

    #[test]
    fn parses_only_supported_npm_fields_and_masks_credentials() {
        let mut diagnostics = Vec::new();
        let values = parse_npmrc(
            "# npm\nregistry=https://person:secret@example.com/npm/\n@team:registry=https://registry.example.com/\ncache=C:\\\\cache\nbad line\n",
            "fixture/.npmrc",
            &mut diagnostics,
        );

        assert_eq!(
            values,
            vec![
                (
                    "@team:registry".into(),
                    "https://registry.example.com/".into()
                ),
                ("registry".into(), "https://***@example.com/npm/".into()),
            ]
        );
        assert_eq!(diagnostics, vec!["fixture/.npmrc 第 5 行格式无法识别。"]);
    }

    #[test]
    fn environment_has_highest_priority_and_preserves_source_trace() {
        let adapter = NpmAdapter::with_sources(
            None,
            BTreeMap::from([
                (
                    "NPM_CONFIG_REGISTRY".into(),
                    "https://registry.environment.example/".into(),
                ),
                ("NPM_CONFIG_CACHE".into(), "C:\\\\cache".into()),
            ]),
        );
        let mut values = BTreeMap::new();
        add_entries(
            &mut values,
            vec![("registry".into(), "https://registry.user.example/".into())],
            ConfigScope::User,
            "C:/Users/example/.npmrc".into(),
            USER_CONFIG_PRIORITY,
        );
        add_entries(
            &mut values,
            vec![(
                "registry".into(),
                "https://registry.project.example/".into(),
            )],
            ConfigScope::Project,
            "C:/work/project/.npmrc".into(),
            PROJECT_CONFIG_PRIORITY,
        );
        add_environment_entries(&mut values, &adapter.environment);

        let registry = &values["registry"];
        assert_eq!(
            registry.value.as_deref(),
            Some("https://registry.environment.example/")
        );
        assert_eq!(
            registry
                .sources
                .iter()
                .map(|source| source.scope)
                .collect::<Vec<_>>(),
            vec![
                ConfigScope::User,
                ConfigScope::Project,
                ConfigScope::Environment
            ]
        );
        assert_eq!(
            registry
                .sources
                .iter()
                .filter_map(|source| source.value.as_deref())
                .collect::<Vec<_>>(),
            vec![
                "https://registry.user.example/",
                "https://registry.project.example/",
                "https://registry.environment.example/"
            ]
        );
    }

    #[test]
    fn creates_a_deterministic_preview_without_writing() {
        let adapter = NpmAdapter::with_sources(None, BTreeMap::new());
        let profile = Profile {
            id: "fixture".into(),
            name: "fixture".into(),
            values: BTreeMap::from([(
                "registry".into(),
                NonSensitiveValue::new("https://registry.next.example/"),
            )]),
        };
        let current_config = ReadResult {
            effective_config: EffectiveConfig {
                tool: ToolId::Npm,
                values: BTreeMap::from([(
                    "registry".into(),
                    EffectiveValue {
                        value: Some("https://registry.environment.example/".into()),
                        sources: vec![
                            ConfigSource {
                                scope: ConfigScope::User,
                                location: "C:/Users/example/.npmrc".into(),
                                priority: USER_CONFIG_PRIORITY,
                                sensitive: false,
                                value: Some("https://registry.user.example/".into()),
                            },
                            ConfigSource {
                                scope: ConfigScope::Environment,
                                location: "NPM_CONFIG_REGISTRY".into(),
                                priority: ENVIRONMENT_PRIORITY,
                                sensitive: false,
                                value: Some("https://registry.environment.example/".into()),
                            },
                        ],
                    },
                )]),
            },
            diagnostics: Vec::new(),
        };

        let plan = adapter
            .plan_for_target(
                Path::new("C:/Users/example/.npmrc"),
                ConfigScope::User,
                USER_CONFIG_PRIORITY,
                &profile,
                &current_config,
            )
            .expect("preview should not require a file to exist");

        assert_eq!(plan.id, "npm-fixture-user");
        assert_eq!(plan.file_checksums["C:/Users/example/.npmrc"], "missing");
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(
            plan.changes[0].previous_value.as_deref(),
            Some("https://registry.user.example/")
        );
        assert_eq!(
            plan.changes[0].next_value.as_deref(),
            Some("https://registry.next.example/")
        );
        assert!(plan.changes[0].risk.is_some());
    }
}

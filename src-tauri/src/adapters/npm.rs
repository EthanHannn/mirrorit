use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckStatus, HealthCheckTarget, Operation, OperationKind, OperationOutcome,
    PlannedChange, Profile, ReadResult, Snapshot, SnapshotFile, SnapshotRef, ToolCapability,
    ToolContext, ToolDetection, ToolId,
};

const USER_CONFIG_PRIORITY: u32 = 100;
const PROJECT_CONFIG_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

#[derive(Debug, Serialize, Deserialize)]
struct NpmSnapshotRecord {
    snapshot: Snapshot,
    original_content: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct NpmAdapter {
    user_config_path: Option<PathBuf>,
    environment: BTreeMap<String, String>,
    snapshot_directory: Option<PathBuf>,
}

impl NpmAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect();
        let user_config_path = user_config_path(&environment);

        Self {
            user_config_path,
            environment,
            snapshot_directory: None,
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
            snapshot_directory: None,
        }
    }

    #[cfg(test)]
    fn with_snapshot_directory(
        user_config_path: Option<PathBuf>,
        environment: BTreeMap<String, String>,
        snapshot_directory: PathBuf,
    ) -> Self {
        Self {
            user_config_path,
            environment,
            snapshot_directory: Some(snapshot_directory),
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

    fn apply(&self, plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        let plan = plan.into_plan();
        if plan.tool != self.tool() {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "变更计划不属于 npm。".into(),
            });
        }
        let (file, expected_checksum) = plan
            .file_checksums
            .iter()
            .next()
            .filter(|_| plan.file_checksums.len() == 1)
            .ok_or_else(|| AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "变更计划必须且只能包含一个目标文件。".into(),
            })?;
        if plan.changes.iter().any(|change| change.file != *file) {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "变更计划包含不一致的目标文件。".into(),
            });
        }
        if file_checksum(Path::new(file))? != *expected_checksum {
            return Err(AdapterError {
                code: AdapterErrorCode::ExternalModification,
                message: "配置文件在预览后发生变化，请重新预览。".into(),
            });
        }

        let path = Path::new(file);
        let original_content = match fs::read(path) {
            Ok(content) => Some(content),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(AdapterError {
                    code: AdapterErrorCode::IoFailure,
                    message: format!("无法读取 {}：{error}", path.display()),
                });
            }
        };
        let content = original_content
            .as_deref()
            .map(|content| {
                String::from_utf8(content.to_vec()).map_err(|_| AdapterError {
                    code: AdapterErrorCode::ParseFailure,
                    message: "npm 配置不是 UTF-8 文本，已拒绝写入。".into(),
                })
            })
            .transpose()?
            .unwrap_or_default();
        let snapshot = self.create_snapshot(path, original_content)?;
        let next_content = update_npmrc(&content, &plan.changes);
        write_atomic(path, next_content.as_bytes(), &snapshot.id)?;

        Ok(ApplyResult {
            operation: Operation {
                id: format!("apply-{}", snapshot.id.0),
                kind: OperationKind::Apply,
                tool: self.tool(),
                outcome: OperationOutcome::Succeeded,
                snapshot: Some(snapshot.id.clone()),
                message: Some("npm 配置已更新，可使用快照回滚。".into()),
            },
            snapshot,
        })
    }

    fn rollback(&self, snapshot: &SnapshotRef) -> AdapterResult<Operation> {
        let record_path = self
            .snapshot_directory()?
            .join(format!("{}.json", snapshot.0));
        let content = fs::read(&record_path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取快照 {}：{error}", snapshot.0),
        })?;
        let record: NpmSnapshotRecord =
            serde_json::from_slice(&content).map_err(|_| AdapterError {
                code: AdapterErrorCode::ParseFailure,
                message: "快照内容无法识别，已拒绝回滚。".into(),
            })?;
        let file = record.snapshot.files.first().ok_or_else(|| AdapterError {
            code: AdapterErrorCode::ParseFailure,
            message: "快照不包含文件记录。".into(),
        })?;
        let path = Path::new(&file.path);
        match record.original_content {
            Some(content) => write_atomic(path, &content, snapshot)?,
            None => match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AdapterError {
                        code: AdapterErrorCode::IoFailure,
                        message: format!("无法移除 {}：{error}", path.display()),
                    });
                }
            },
        }

        Ok(Operation {
            id: format!("rollback-{}", snapshot.0),
            kind: OperationKind::Rollback,
            tool: self.tool(),
            outcome: OperationOutcome::Succeeded,
            snapshot: Some(snapshot.clone()),
            message: Some("npm 配置已从快照恢复。".into()),
        })
    }

    fn health_check(&self, target: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        let address = target.address.trim();
        if !(address.starts_with("https://") || address.starts_with("http://")) {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "检查地址必须以 http:// 或 https:// 开头。".into(),
            });
        }
        let started_at = Instant::now();
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: format!("无法创建检查请求：{error}"),
            })?;

        let result = match client.get(address).send() {
            Ok(response) => {
                let status = classify_http_status(response.status().as_u16());
                HealthCheckResult {
                    target: target.clone(),
                    status,
                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                    message: Some(format!("HTTP {}", response.status().as_u16())),
                }
            }
            Err(error) => HealthCheckResult {
                target: target.clone(),
                status: classify_request_error(&error),
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                message: Some("请求未完成；请检查网络、证书或代理设置。".into()),
            },
        };

        Ok(result)
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
            target_checksums: BTreeMap::new(),
            file_checksums: BTreeMap::from([(file, file_checksum)]),
            changes,
        })
    }

    fn create_snapshot(
        &self,
        path: &Path,
        original_content: Option<Vec<u8>>,
    ) -> AdapterResult<Snapshot> {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: "系统时间不可用，无法创建快照。".into(),
            })?
            .as_millis() as i64;
        let snapshot = Snapshot {
            id: SnapshotRef(format!("npm-{created_at_ms}")),
            created_at_ms,
            files: vec![SnapshotFile {
                path: path.display().to_string(),
                checksum: original_content
                    .as_deref()
                    .map(|content| format!("{:x}", Sha256::digest(content)))
                    .unwrap_or_else(|| "missing".into()),
                permissions: None,
            }],
        };
        let directory = self.snapshot_directory()?;
        fs::create_dir_all(&directory).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法创建快照目录：{error}"),
        })?;
        let record = NpmSnapshotRecord {
            snapshot: snapshot.clone(),
            original_content,
        };
        let content = serde_json::to_vec(&record).map_err(|_| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: "无法序列化快照。".into(),
        })?;
        fs::write(directory.join(format!("{}.json", snapshot.id.0)), content).map_err(|error| {
            AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: format!("无法保存快照：{error}"),
            }
        })?;

        Ok(snapshot)
    }

    fn snapshot_directory(&self) -> AdapterResult<PathBuf> {
        if let Some(directory) = &self.snapshot_directory {
            return Ok(directory.clone());
        }
        let base = std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("APPDATA"))
            .map_err(|_| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: "未能定位本地应用数据目录。".into(),
            })?;
        Ok(Path::new(&base).join("MirrorIt").join("snapshots"))
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
        ConfigScope::VirtualEnvironment => "virtual_environment",
        ConfigScope::Environment => "environment",
    }
}

fn update_npmrc(content: &str, changes: &[PlannedChange]) -> String {
    let mut lines = content
        .split_inclusive('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    for change in changes {
        let replacement = change
            .next_value
            .as_ref()
            .map(|value| format!("{}={value}", change.field));
        let index = lines.iter().rposition(|line| {
            line.split_once('=')
                .is_some_and(|(key, _)| normalize_key(key) == change.field)
        });
        match (index, replacement) {
            (Some(index), Some(replacement)) => {
                let line_ending = if lines[index].ends_with("\r\n") {
                    "\r\n"
                } else if lines[index].ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                lines[index] = format!("{replacement}{line_ending}");
            }
            (Some(index), None) => {
                lines.remove(index);
            }
            (None, Some(replacement)) => {
                if !lines.is_empty() && !lines.last().is_some_and(|line| line.ends_with('\n')) {
                    lines.push("\n".into());
                }
                lines.push(format!("{replacement}\n"));
            }
            (None, None) => {}
        }
    }

    lines.concat()
}

fn write_atomic(path: &Path, content: &[u8], snapshot: &SnapshotRef) -> AdapterResult<()> {
    let parent = path.parent().ok_or_else(|| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: "目标文件没有父目录。".into(),
    })?;
    fs::create_dir_all(parent).map_err(|error| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: format!("无法创建目标目录：{error}"),
    })?;
    let temporary_path = parent.join(format!(".mirrorit-{}.tmp", snapshot.0));
    fs::write(&temporary_path, content).map_err(|error| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: format!("无法写入临时配置：{error}"),
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法替换现有配置：{error}"),
        })?;
    }
    fs::rename(&temporary_path, path).map_err(|error| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: format!("无法完成配置替换：{error}"),
    })
}

fn classify_http_status(status: u16) -> HealthCheckStatus {
    if matches!(status, 401 | 403) {
        HealthCheckStatus::AuthenticationFailure
    } else if (200..400).contains(&status) {
        HealthCheckStatus::Healthy
    } else {
        HealthCheckStatus::HttpFailure
    }
}

fn classify_request_error(error: &reqwest::Error) -> HealthCheckStatus {
    if error.is_timeout() {
        return HealthCheckStatus::Timeout;
    }
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("dns") || text.contains("name or service not known") {
        HealthCheckStatus::DnsFailure
    } else if text.contains("tls") || text.contains("certificate") || text.contains("ssl") {
        HealthCheckStatus::TlsFailure
    } else {
        HealthCheckStatus::HttpFailure
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn applies_a_preview_with_snapshot_and_restores_it() {
        let root = test_directory("apply-rollback");
        let config_path = root.join(".npmrc");
        let snapshot_directory = root.join("snapshots");
        fs::write(
            &config_path,
            "cache=C:\\\\npm-cache\nregistry=https://registry.previous.example/\n",
        )
        .expect("fixture config should be written");
        let adapter = NpmAdapter::with_snapshot_directory(
            Some(config_path.clone()),
            BTreeMap::new(),
            snapshot_directory,
        );
        let current_config = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("fixture config should be readable");
        let profile = Profile {
            id: "fixture".into(),
            name: "fixture".into(),
            values: BTreeMap::from([(
                "registry".into(),
                NonSensitiveValue::new("https://registry.next.example/"),
            )]),
        };
        let plan = adapter
            .plan_for_target(
                &config_path,
                ConfigScope::User,
                USER_CONFIG_PRIORITY,
                &profile,
                &current_config,
            )
            .expect("preview should succeed");

        let applied = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect("confirmed plan should apply");
        assert_eq!(
            fs::read_to_string(&config_path).expect("updated config should be readable"),
            "cache=C:\\\\npm-cache\nregistry=https://registry.next.example/\n"
        );

        adapter
            .rollback(&applied.snapshot.id)
            .expect("snapshot should restore");
        assert_eq!(
            fs::read_to_string(&config_path).expect("restored config should be readable"),
            "cache=C:\\\\npm-cache\nregistry=https://registry.previous.example/\n"
        );

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn rejects_a_preview_when_the_target_changes_externally() {
        let root = test_directory("external-change");
        let config_path = root.join(".npmrc");
        fs::write(
            &config_path,
            "registry=https://registry.previous.example/\n",
        )
        .expect("fixture config should be written");
        let adapter = NpmAdapter::with_snapshot_directory(
            Some(config_path.clone()),
            BTreeMap::new(),
            root.join("snapshots"),
        );
        let current_config = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("fixture config should be readable");
        let profile = Profile {
            id: "fixture".into(),
            name: "fixture".into(),
            values: BTreeMap::from([(
                "registry".into(),
                NonSensitiveValue::new("https://registry.next.example/"),
            )]),
        };
        let plan = adapter
            .plan_for_target(
                &config_path,
                ConfigScope::User,
                USER_CONFIG_PRIORITY,
                &profile,
                &current_config,
            )
            .expect("preview should succeed");
        fs::write(
            &config_path,
            "registry=https://registry.external.example/\n",
        )
        .expect("external change should be written");

        let error = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect_err("changed target must reject the plan");
        assert_eq!(error.code, AdapterErrorCode::ExternalModification);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn classifies_http_responses_without_network_access() {
        assert_eq!(classify_http_status(200), HealthCheckStatus::Healthy);
        assert_eq!(
            classify_http_status(403),
            HealthCheckStatus::AuthenticationFailure
        );
        assert_eq!(classify_http_status(503), HealthCheckStatus::HttpFailure);
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

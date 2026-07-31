use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use serde::Deserialize;

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, ReadResult, SnapshotRef, ToolCapability, ToolContext,
    ToolDetection, ToolId,
};

const EFFECTIVE_PRIORITY: u32 = 100;
const USER_CONFIG_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;
const SUPPORTED_KEYS: [&str; 4] = ["GOPROXY", "GONOSUMDB", "GOPRIVATE", "GOSUMDB"];

#[derive(Debug, Default, Deserialize)]
struct GoEnvironment {
    #[serde(rename = "GOENV")]
    goenv: String,
    #[serde(rename = "GONOSUMDB")]
    gonosumdb: String,
    #[serde(rename = "GOPRIVATE")]
    goprivate: String,
    #[serde(rename = "GOPROXY")]
    goproxy: String,
    #[serde(rename = "GOSUMDB")]
    gosumdb: String,
}

impl GoEnvironment {
    fn value(&self, key: &str) -> &str {
        match key {
            "GONOSUMDB" => &self.gonosumdb,
            "GOPRIVATE" => &self.goprivate,
            "GOPROXY" => &self.goproxy,
            "GOSUMDB" => &self.gosumdb,
            _ => "",
        }
    }
}

trait GoCommandRunner: Send + Sync {
    fn version(&self) -> Result<String, String>;
    fn environment(&self) -> Result<String, String>;
}

struct SystemGoCommandRunner;

impl GoCommandRunner for SystemGoCommandRunner {
    fn version(&self) -> Result<String, String> {
        let output = Command::new("go")
            .arg("version")
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn environment(&self) -> Result<String, String> {
        let output = Command::new("go")
            .args([
                "env",
                "-json",
                "GOENV",
                "GOPROXY",
                "GONOSUMDB",
                "GOPRIVATE",
                "GOSUMDB",
            ])
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        String::from_utf8(output.stdout).map_err(|error| error.to_string())
    }
}

pub struct GoAdapter {
    environment: BTreeMap<String, String>,
    command_runner: Arc<dyn GoCommandRunner>,
}

impl GoAdapter {
    pub fn from_system() -> Self {
        Self {
            environment: std::env::vars().collect(),
            command_runner: Arc::new(SystemGoCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_sources(
        environment: BTreeMap<String, String>,
        command_runner: Arc<dyn GoCommandRunner>,
    ) -> Self {
        Self {
            environment,
            command_runner,
        }
    }

    fn read_goenv(
        &self,
        path: &Path,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        match fs::read_to_string(path) {
            Ok(content) => {
                for (key, value) in parse_goenv(&content, diagnostics) {
                    add_source(
                        values,
                        &key,
                        value,
                        ConfigScope::User,
                        path.display().to_string(),
                        USER_CONFIG_PRIORITY,
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => diagnostics.push(format!("无法读取 {}：{error}", path.display())),
        }
    }
}

impl ConfigAdapter for GoAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Go
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
        let mut diagnostics = Vec::new();
        let output = match self.command_runner.environment() {
            Ok(output) => output,
            Err(error) => {
                return Ok(ReadResult {
                    effective_config: EffectiveConfig {
                        tool: self.tool(),
                        values: BTreeMap::new(),
                    },
                    diagnostics: vec![format!(
                        "无法读取 Go 环境；请确认 Go 已安装并可在终端中运行：{}",
                        concise_error(&error)
                    )],
                });
            }
        };
        let go_environment =
            serde_json::from_str::<GoEnvironment>(&output).map_err(|error| AdapterError {
                code: AdapterErrorCode::ParseFailure,
                message: format!("Go 返回了无法解析的环境信息：{error}"),
            })?;

        let mut values = BTreeMap::new();
        for key in SUPPORTED_KEYS {
            add_source(
                &mut values,
                key,
                go_environment.value(key).to_owned(),
                ConfigScope::System,
                "go env".into(),
                EFFECTIVE_PRIORITY,
            );
        }

        if go_environment.goenv == "off" {
            diagnostics.push("GOENV 已设为 off，已跳过用户级 Go 环境配置文件。".into());
        } else if go_environment.goenv.is_empty() {
            diagnostics.push("Go 未返回 GOENV 路径，无法解释用户级配置来源。".into());
        } else {
            self.read_goenv(
                Path::new(&go_environment.goenv),
                &mut values,
                &mut diagnostics,
            );
        }

        for key in SUPPORTED_KEYS {
            if let Some(value) = self.environment.get(key) {
                add_source(
                    &mut values,
                    key,
                    value.clone(),
                    ConfigScope::Environment,
                    format!("环境变量 {key}"),
                    ENVIRONMENT_PRIORITY,
                );
            }
        }

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

fn parse_goenv(content: &str, diagnostics: &mut Vec<String>) -> Vec<(String, String)> {
    let mut values = Vec::new();
    for (line_number, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            diagnostics.push(format!("GOENV 第 {} 行格式无效，已忽略。", line_number + 1));
            continue;
        };
        if SUPPORTED_KEYS.contains(&key) {
            values.push((key.to_owned(), value.to_owned()));
        }
    }
    values
}

fn add_source(
    values: &mut BTreeMap<String, EffectiveValue>,
    key: &str,
    value: String,
    scope: ConfigScope,
    location: String,
    priority: u32,
) {
    let redacted_value = redact_url_credentials(&value);
    let sensitive = redacted_value != value;
    let entry = values
        .entry(key.to_owned())
        .or_insert_with(|| EffectiveValue {
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
    value
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            let Some(scheme_end) = trimmed.find("://") else {
                return part.to_owned();
            };
            let authority_start = scheme_end + 3;
            let Some(at_offset) = trimmed[authority_start..].find('@') else {
                return part.to_owned();
            };
            let at = authority_start + at_offset;
            format!("{}***:***{}", &trimmed[..authority_start], &trimmed[at..])
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn concise_error(error: &str) -> &str {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        "未知错误"
    } else {
        trimmed
    }
}

fn read_only_error() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "Go 当前仅支持只读扫描；不会修改 GOENV 或环境变量。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct FixtureRunner {
        version: Result<String, String>,
        environment: Result<String, String>,
    }

    impl GoCommandRunner for FixtureRunner {
        fn version(&self) -> Result<String, String> {
            self.version.clone()
        }

        fn environment(&self) -> Result<String, String> {
            self.environment.clone()
        }
    }

    fn runner(environment: String) -> Arc<dyn GoCommandRunner> {
        Arc::new(FixtureRunner {
            version: Ok("go version go1.26.5 windows/amd64".into()),
            environment: Ok(environment),
        })
    }

    fn environment_json(goenv: &Path) -> String {
        format!(
            r#"{{"GOENV":"{}","GOPROXY":"https://proxy.example,direct","GONOSUMDB":"","GOPRIVATE":"","GOSUMDB":"sum.golang.org"}}"#,
            goenv.display().to_string().replace('\\', "\\\\")
        )
    }

    #[test]
    fn reads_effective_values_and_explains_goenv_and_environment_sources() {
        let directory = test_directory("source-precedence");
        let goenv = directory.join("env");
        fs::write(
            &goenv,
            "GOPROXY=https://user.example,direct\nGOPRIVATE=*.corp.example\n",
        )
        .expect("fixture GOENV");
        let adapter = GoAdapter::with_sources(
            BTreeMap::from([("GOPROXY".into(), "https://environment.example".into())]),
            runner(environment_json(&goenv)),
        );

        let result = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(
            result.effective_config.values["GOPROXY"].value.as_deref(),
            Some("https://environment.example")
        );
        assert_eq!(result.effective_config.values["GOPROXY"].sources.len(), 3);
        assert_eq!(
            result.effective_config.values["GOPRIVATE"].value.as_deref(),
            Some("*.corp.example")
        );

        fs::remove_dir_all(directory).expect("fixture directory should be removed");
    }

    #[test]
    fn reports_invalid_goenv_lines_without_aborting_scan() {
        let directory = test_directory("invalid-goenv");
        let goenv = directory.join("env");
        fs::write(&goenv, "not valid\nGOSUMDB=sum.golang.org\n").expect("fixture GOENV");
        let adapter = GoAdapter::with_sources(BTreeMap::new(), runner(environment_json(&goenv)));

        let result = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(
            result.effective_config.values["GOSUMDB"].value.as_deref(),
            Some("sum.golang.org")
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|message| message.contains("第 1 行格式无效")));

        fs::remove_dir_all(directory).expect("fixture directory should be removed");
    }

    #[test]
    fn skips_user_file_when_goenv_is_disabled() {
        let adapter = GoAdapter::with_sources(
            BTreeMap::new(),
            runner(
                r#"{"GOENV":"off","GOPROXY":"https://proxy.example,direct","GONOSUMDB":"","GOPRIVATE":"","GOSUMDB":"sum.golang.org"}"#.into(),
            ),
        );

        let result = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        assert_eq!(result.effective_config.values["GOPROXY"].sources.len(), 1);
        assert!(result
            .diagnostics
            .iter()
            .any(|message| message.contains("GOENV 已设为 off")));
    }

    #[test]
    fn reports_missing_go_without_failing_the_read_operation() {
        let adapter = GoAdapter::with_sources(
            BTreeMap::new(),
            Arc::new(FixtureRunner {
                version: Err("not found".into()),
                environment: Err("program not found".into()),
            }),
        );

        let result = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("missing Go should be reported as a diagnostic");

        assert!(result.effective_config.values.is_empty());
        assert!(result
            .diagnostics
            .iter()
            .any(|message| message.contains("无法读取 Go 环境")));
    }

    #[test]
    fn rejects_malformed_go_json() {
        let adapter =
            GoAdapter::with_sources(BTreeMap::new(), runner("{ this is not valid json".into()));

        let error = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect_err("malformed JSON must fail");

        assert_eq!(error.code, AdapterErrorCode::ParseFailure);
    }

    #[test]
    fn redacts_credentials_before_returning_values() {
        let adapter = GoAdapter::with_sources(
            BTreeMap::new(),
            runner(
                r#"{"GOENV":"off","GOPROXY":"https://alice:secret@proxy.example,direct","GONOSUMDB":"","GOPRIVATE":"","GOSUMDB":"sum.golang.org"}"#.into(),
            ),
        );

        let result = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("read result");

        let value = &result.effective_config.values["GOPROXY"];
        assert_eq!(
            value.value.as_deref(),
            Some("https://***:***@proxy.example,direct")
        );
        assert!(value.sources[0].sensitive);
    }

    fn test_directory(name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-go-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

use std::collections::{BTreeMap, BTreeSet};
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

const SYSTEM_PRIORITY: u32 = 50;
const USER_PRIORITY: u32 = 100;
const VIRTUAL_ENVIRONMENT_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

trait PipCommandRunner: Send + Sync {
    fn version(&self) -> Result<String, String>;
    fn site_prefix(&self) -> Result<String, String>;
}

struct SystemPipCommandRunner;

impl PipCommandRunner for SystemPipCommandRunner {
    fn version(&self) -> Result<String, String> {
        run_python(&["-m", "pip", "--version"])
    }

    fn site_prefix(&self) -> Result<String, String> {
        run_python(&["-c", "import sys; print(sys.prefix)"])
    }
}

fn run_python(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("python")
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub struct PipAdapter {
    environment: BTreeMap<String, String>,
    global_paths: Vec<PathBuf>,
    user_paths: Vec<PathBuf>,
    command_runner: Arc<dyn PipCommandRunner>,
}

impl PipAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect::<BTreeMap<_, _>>();
        Self {
            global_paths: global_paths(&environment),
            user_paths: user_paths(&environment),
            environment,
            command_runner: Arc::new(SystemPipCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_sources(
        global_paths: Vec<PathBuf>,
        user_paths: Vec<PathBuf>,
        environment: BTreeMap<String, String>,
        command_runner: Arc<dyn PipCommandRunner>,
    ) -> Self {
        Self {
            environment,
            global_paths,
            user_paths,
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
        add_entries(
            values,
            parse_ini(&content, &path.display().to_string(), diagnostics),
            scope,
            path.display().to_string(),
            priority,
        );
    }
}

impl ConfigAdapter for PipAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Pip
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

        if self.command_runner.version().is_err() {
            diagnostics.push("未检测到 pip；仍会读取 Windows 约定配置路径与环境变量。".into());
        }
        for path in &self.global_paths {
            self.read_file(
                path,
                ConfigScope::System,
                SYSTEM_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        }
        for path in &self.user_paths {
            self.read_file(
                path,
                ConfigScope::User,
                USER_PRIORITY,
                &mut values,
                &mut diagnostics,
            );
        }
        match self.command_runner.site_prefix() {
            Ok(prefix) if !prefix.is_empty() => self.read_file(
                &Path::new(&prefix).join("pip.ini"),
                ConfigScope::VirtualEnvironment,
                VIRTUAL_ENVIRONMENT_PRIORITY,
                &mut values,
                &mut diagnostics,
            ),
            _ => diagnostics.push("未能定位当前 Python 的 site pip.ini，已跳过该来源。".into()),
        }
        if self.environment.contains_key("PIP_CONFIG_FILE") {
            diagnostics
                .push("为避免读取约定范围外的文件，未读取 PIP_CONFIG_FILE 指向的配置。".into());
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

fn global_paths(environment: &BTreeMap<String, String>) -> Vec<PathBuf> {
    environment
        .get("PROGRAMDATA")
        .filter(|path| !path.trim().is_empty())
        .map(|directory| vec![Path::new(directory).join("pip").join("pip.ini")])
        .unwrap_or_default()
}

fn user_paths(environment: &BTreeMap<String, String>) -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    if let Some(directory) = environment
        .get("APPDATA")
        .filter(|path| !path.trim().is_empty())
    {
        paths.insert(Path::new(directory).join("pip").join("pip.ini"));
    }
    if let Some(directory) = environment
        .get("USERPROFILE")
        .or_else(|| environment.get("HOME"))
        .filter(|path| !path.trim().is_empty())
    {
        paths.insert(Path::new(directory).join("pip").join("pip.ini"));
    }
    paths.into_iter().collect()
}

fn parse_ini(
    content: &str,
    location: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<(String, String)> {
    let mut by_section = BTreeMap::new();
    let mut section = None;

    for (line_number, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(line[1..line.len() - 1].trim().to_ascii_lowercase());
            continue;
        }
        if !matches!(section.as_deref(), Some("global" | "install")) {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            diagnostics.push(format!(
                "{location} 第 {} 行的 pip 配置格式无法识别。",
                line_number + 1
            ));
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase().replace('_', "-");
        if is_supported_key(&key) {
            by_section.insert(
                (section.clone().expect("section should be available"), key),
                redact_credentials(raw_value.trim()),
            );
        }
    }

    ["global", "install"]
        .into_iter()
        .flat_map(|section| {
            by_section
                .iter()
                .filter(move |((current_section, _), _)| current_section == section)
                .map(|((_, key), value)| (key.clone(), value.clone()))
        })
        .collect()
}

fn add_environment_entries(
    values: &mut BTreeMap<String, EffectiveValue>,
    environment: &BTreeMap<String, String>,
) {
    for (name, key) in [
        ("PIP_INDEX_URL", "index-url"),
        ("PIP_EXTRA_INDEX_URL", "extra-index-url"),
        ("PIP_TRUSTED_HOST", "trusted-host"),
        ("PIP_PROXY", "proxy"),
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

fn is_supported_key(key: &str) -> bool {
    matches!(
        key,
        "index-url" | "extra-index-url" | "trusted-host" | "proxy"
    )
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
        message: "pip 当前仅支持只读扫描；不会修改配置、凭据或缓存。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct FixtureRunner {
        version: Result<String, String>,
        site_prefix: Result<String, String>,
    }

    impl PipCommandRunner for FixtureRunner {
        fn version(&self) -> Result<String, String> {
            self.version.clone()
        }

        fn site_prefix(&self) -> Result<String, String> {
            self.site_prefix.clone()
        }
    }

    fn adapter(
        global_paths: Vec<PathBuf>,
        user_paths: Vec<PathBuf>,
        environment: BTreeMap<String, String>,
        site_prefix: Result<String, String>,
    ) -> PipAdapter {
        PipAdapter::with_sources(
            global_paths,
            user_paths,
            environment,
            Arc::new(FixtureRunner {
                version: Ok("pip 25.2".into()),
                site_prefix,
            }),
        )
    }

    #[test]
    fn reads_global_user_site_and_environment_sources_in_order() {
        let root = test_directory("precedence");
        let global = root.join("global.ini");
        let user = root.join("user.ini");
        let site = root.join("site");
        fs::create_dir_all(&site).expect("site directory");
        fs::write(&global, "[global]\nindex-url = https://global.example/\n")
            .expect("global config");
        fs::write(&user, "[global]\nindex-url = https://user.example/\n").expect("user config");
        fs::write(
            site.join("pip.ini"),
            "[global]\nindex-url = https://site.example/\n",
        )
        .expect("site config");

        let result = adapter(
            vec![global],
            vec![user],
            BTreeMap::from([(
                "PIP_INDEX_URL".into(),
                "https://environment.example/".into(),
            )]),
            Ok(site.display().to_string()),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["index-url"].value.as_deref(),
            Some("https://environment.example/")
        );
        assert_eq!(result.effective_config.values["index-url"].sources.len(), 4);
        assert_eq!(
            result.effective_config.values["index-url"].sources[2].scope,
            ConfigScope::VirtualEnvironment
        );

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn install_section_overrides_global_in_the_same_file() {
        let root = test_directory("sections");
        let user = root.join("user.ini");
        fs::write(
            &user,
            "[global]\nproxy = https://global.example/\n[install]\nproxy = https://install.example/\n",
        )
        .expect("user config");

        let result = adapter(
            Vec::new(),
            vec![user],
            BTreeMap::new(),
            Err("no site".into()),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(
            result.effective_config.values["proxy"].value.as_deref(),
            Some("https://install.example/")
        );
        assert_eq!(result.effective_config.values["proxy"].sources.len(), 2);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn reports_malformed_ini_and_keeps_environment_values() {
        let root = test_directory("invalid");
        let user = root.join("user.ini");
        fs::write(&user, "[global]\nindex-url").expect("user config");

        let result = adapter(
            Vec::new(),
            vec![user],
            BTreeMap::from([("PIP_PROXY".into(), "https://proxy.example/".into())]),
            Err("no site".into()),
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
        assert_eq!(result.diagnostics.len(), 2);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn skips_arbitrary_pip_config_file_and_redacts_credentials() {
        let root = test_directory("credentials");
        let user = root.join("user.ini");
        fs::write(
            &user,
            "[global]\nindex-url = https://alice:secret@registry.example/\nclient-cert = private.pem\n",
        )
        .expect("user config");

        let result = adapter(
            Vec::new(),
            vec![user],
            BTreeMap::from([("PIP_CONFIG_FILE".into(), "C:\\secret.ini".into())]),
            Err("no site".into()),
        )
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .expect("read result");

        assert_eq!(result.effective_config.values.len(), 1);
        assert_eq!(
            result.effective_config.values["index-url"].value.as_deref(),
            Some("https://***@registry.example/")
        );
        assert!(result.effective_config.values["index-url"].sources[0].sensitive);
        assert_eq!(result.diagnostics.len(), 2);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-pip-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}

use serde::Deserialize;
use std::path::{Path, PathBuf};

const RESERVED_MOUNT_PATHS: &[&str] = &[
    "assets", "data", "media", "api", "search", "s", "admin", "lobby", "theme",
];

#[derive(Debug, Clone, Deserialize)]
pub struct MountConfig {
    pub id: String,
    pub source: PathBuf,
    pub path: String,
    pub title_ko: String,
    pub title_en: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub open_in_new_tab: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub site: SiteConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub extensions: ExtensionsConfig,
    #[serde(default)]
    pub integrations: IntegrationsConfig,
    #[serde(default)]
    pub lobby: LobbySection,
    #[serde(default)]
    pub deploy: crate::site_paths::DeployConfig,
    #[serde(default)]
    pub mounts: Vec<MountConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            site: SiteConfig {
                name: "Oxibuilder".into(),
                base_url: "http://127.0.0.1:8787".into(),
                default_lang: default_lang(),
                languages: default_languages(),
            },
            server: ServerConfig::default(),
            extensions: ExtensionsConfig::default(),
            integrations: IntegrationsConfig::default(),
            lobby: LobbySection::default(),
            deploy: crate::site_paths::DeployConfig::default(),
            mounts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_lang")]
    pub default_lang: String,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

fn default_lang() -> String {
    "ko".into()
}

fn default_languages() -> Vec<String> {
    vec!["ko".into(), "en".into()]
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub api_endpoint: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "127.0.0.1".into(),
            port: 8787,
            data_dir: PathBuf::from("data"),
            api_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// doc/04 §4.4 [integrations]. 값 자체가 아닌 환경변수 이름을 저장 (git 커밋 안전).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IntegrationsConfig {
    #[serde(default)]
    pub github_username: Option<String>,
    #[serde(default)]
    pub tmdb_api_key_env: Option<String>,
    #[serde(default)]
    pub aladin_ttbkey_env: Option<String>,
}

impl IntegrationsConfig {
    /// github_username을 TOML에서 읽거나 OXIBUILDER_GITHUB_USERNAME 폴백.
    pub fn github_username(&self) -> Option<String> {
        self.github_username
            .clone()
            .or_else(|| std::env::var("OXIBUILDER_GITHUB_USERNAME").ok())
            .filter(|s| !s.is_empty())
    }

    /// tmdb_api_key_env가 가리키는 환경변수에서 키 값을 읽거나 OXIBUILDER_TMDB_KEY 폴백.
    pub fn tmdb_key(&self) -> Option<String> {
        let env_name = self
            .tmdb_api_key_env
            .as_deref()
            .unwrap_or("OXIBUILDER_TMDB_KEY");
        std::env::var(env_name).ok().filter(|s| !s.is_empty())
    }

    /// 알라딘 TTBKey. 환경변수 이름 또는 OXIBUILDER_ALADIN_TTBKEY 폴백.
    pub fn aladin_key(&self) -> Option<String> {
        let env_name = self
            .aladin_ttbkey_env
            .as_deref()
            .unwrap_or("OXIBUILDER_ALADIN_TTBKEY");
        std::env::var(env_name).ok().filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LobbySection {
    pub default_mode: String,
    pub layout: String,
}

impl Default for LobbySection {
    fn default() -> Self {
        Self {
            default_mode: "grid".to_string(),
            layout: "shell".to_string(),
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("failed to parse config file {0}: {1}")]
    Parse(PathBuf, toml::de::Error),
    #[error("invalid [[mounts]] config: {0}")]
    InvalidMounts(String),
}

impl Config {
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;
        let mut cfg: Config =
            toml::from_str(&raw).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))?;
        cfg.apply_env_overrides();
        cfg.validate_mounts()
            .map_err(ConfigError::InvalidMounts)?;
        cfg.resolve_mount_sources(path.parent().unwrap_or_else(|| Path::new(".")));
        Ok(cfg)
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(port) = std::env::var("OXIBUILDER_PORT")
            && let Ok(port) = port.parse::<u16>()
        {
            self.server.port = port;
        }
        if let Ok(dir) = std::env::var("OXIBUILDER_DATA_DIR") {
            self.server.data_dir = PathBuf::from(dir);
        }
    }

    /// Structural validation of `[[mounts]]`: unique ids/paths, no reserved
    /// prefixes, no `..`/`.`/absolute paths. Pure (no filesystem access).
    pub fn validate_mounts(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for m in &self.mounts {
            if !ids.insert(&m.id) {
                return Err(format!("duplicate mount id: {}", m.id));
            }
            let norm = m.path.trim_matches('/');
            if norm.is_empty() {
                return Err(format!("mount {} has empty path", m.id));
            }
            if norm
                .split('/')
                .any(|seg| seg == ".." || seg == ".")
            {
                return Err(format!("mount {} has invalid path: {}", m.id, m.path));
            }
            let top = norm.split('/').next().unwrap();
            if RESERVED_MOUNT_PATHS.contains(&top) {
                return Err(format!(
                    "mount {} uses reserved path prefix: {}",
                    m.id, top
                ));
            }
            if !paths.insert(norm) {
                return Err(format!("duplicate mount path: {}", m.path));
            }
        }
        Ok(())
    }
    /// Validate [lobby].layout against the known layout catalog.
    pub fn validate_layout(&self) -> Result<(), String> {
        if !crate::theme::is_known_layout(&self.lobby.layout) {
            return Err(format!(
                "'{}' is not a valid [lobby].layout (expected 'shell' or 'editorial')",
                self.lobby.layout
            ));
        }
        Ok(())
    }

    /// Resolve each mount's `source` to an absolute path relative to `base`.
    /// Warns (non-fatal) when a source dir does not exist.
    pub fn resolve_mount_sources(&mut self, base: &Path) {
        for m in &mut self.mounts {
            if !m.source.is_absolute() {
                m.source = base.join(&m.source);
            }
            if !m.source.is_dir() {
                tracing::warn!(
                    "mount {} source not found: {}",
                    m.id,
                    m.source.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config_with_defaults() {
        let cfg = Config::from_toml_str(
            r#"
[site]
name = "테스트 작업실"
base_url = "https://example.dev"
"#,
        )
        .unwrap();
        assert_eq!(cfg.site.name, "테스트 작업실");
        assert_eq!(cfg.site.default_lang, "ko");
        assert_eq!(cfg.site.languages, vec!["ko", "en"]);
        assert_eq!(cfg.server.port, 8787);
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert!(cfg.extensions.enabled.is_empty());
        assert_eq!(cfg.lobby.default_mode, "grid");
    }

    #[test]
    fn lobby_layout_defaults_to_shell() {
        let cfg = Config::default();
        assert_eq!(cfg.lobby.layout, "shell");
    }

    #[test]
    fn lobby_layout_rejects_unknown() {
        let toml = "[site]\nname = \"Test\"\nbase_url = \"https://example.test\"\n\n[lobby]\nlayout = \"bogus\"\n";
        let cfg = toml::from_str::<Config>(toml).expect("parses");
        assert!(cfg.validate_layout().is_err());
    }

    #[test]
    fn parses_full_config() {
        let cfg = Config::from_toml_str(
            r#"
[site]
name = "S"
base_url = "https://b.dev"
default_lang = "en"
languages = ["en", "ko"]

[server]
port = 9999
data_dir = "/var/oxibuilder"

[extensions]
enabled = ["profile", "blog"]

[lobby]
default_mode = "canvas"
"#,
        )
        .unwrap();
        assert_eq!(cfg.site.default_lang, "en");
        assert_eq!(cfg.server.port, 9999);
        assert_eq!(
            cfg.server.data_dir,
            std::path::PathBuf::from("/var/oxibuilder")
        );
        assert_eq!(cfg.extensions.enabled, vec!["profile", "blog"]);
        assert_eq!(cfg.lobby.default_mode, "canvas");
    }

    #[test]
    fn rejects_invalid_toml() {
        assert!(Config::from_toml_str("not [valid").is_err());
    }

    #[test]
    fn env_overrides_port_and_data_dir() {
        unsafe {
            std::env::set_var("OXIBUILDER_PORT", "1234");
            std::env::set_var("OXIBUILDER_DATA_DIR", "/tmp/oxi-test");
        }
        let mut cfg = Config::default();
        cfg.apply_env_overrides();
        assert_eq!(cfg.server.port, 1234);
        assert_eq!(
            cfg.server.data_dir,
            std::path::PathBuf::from("/tmp/oxi-test")
        );
        unsafe {
            std::env::remove_var("OXIBUILDER_PORT");
            std::env::remove_var("OXIBUILDER_DATA_DIR");
        }
    }

    #[test]
    fn parses_mounts_section() {
        let cfg = Config::from_toml_str(
            r#"
[site]
name = "S"
base_url = "https://b.dev"

[[mounts]]
id = "portfolio"
source = "../portfolio"
path = "portfolio"
title_ko = "포트폴리오"
title_en = "Portfolio"
description = "Hand-crafted work"
"#,
        )
        .unwrap();
        assert_eq!(cfg.mounts.len(), 1);
        let m = &cfg.mounts[0];
        assert_eq!(m.id, "portfolio");
        assert_eq!(m.path, "portfolio");
        assert_eq!(m.title_en, "Portfolio");
        assert_eq!(m.description.as_deref(), Some("Hand-crafted work"));
        assert!(!m.open_in_new_tab); // default false
    }

    #[test]
    fn validate_rejects_reserved_path_prefix() {
        let mut cfg = Config::default();
        cfg.mounts.push(MountConfig {
            id: "x".into(), source: "/a".into(), path: "assets".into(),
            title_ko: "k".into(), title_en: "e".into(),
            description: None, icon: None, open_in_new_tab: false,
        });
        assert!(cfg.validate_mounts().is_err());
    }

    #[test]
    fn validate_rejects_duplicate_id() {
        let mut cfg = Config::default();
        for _ in 0..2 {
            cfg.mounts.push(MountConfig {
                id: "dup".into(), source: "/a".into(), path: "a".into(),
                title_ko: "k".into(), title_en: "e".into(),
                description: None, icon: None, open_in_new_tab: false,
            });
        }
        let err = cfg.validate_mounts().unwrap_err();
        assert!(err.contains("duplicate mount id"), "{err}");
    }

    #[test]
    fn resolve_mount_sources_makes_relative_absolute() {
        let mut cfg = Config::default();
        cfg.mounts.push(MountConfig {
            id: "p".into(), source: "../portfolio".into(), path: "portfolio".into(),
            title_ko: "k".into(), title_en: "e".into(),
            description: None, icon: None, open_in_new_tab: false,
        });
        let base = std::path::Path::new("/srv/oxibuilder");
        cfg.resolve_mount_sources(base);
        assert_eq!(cfg.mounts[0].source, std::path::PathBuf::from("/srv/oxibuilder/../portfolio"));
    }
}


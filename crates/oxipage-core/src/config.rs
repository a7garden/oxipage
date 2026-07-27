use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub site: SiteConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub extensions: ExtensionsConfig,
    #[serde(default)]
    pub lobby: LobbySection,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            site: SiteConfig {
                name: "Oxipage".into(),
                base_url: "http://127.0.0.1:8787".into(),
                default_lang: default_lang(),
                languages: default_languages(),
            },
            server: ServerConfig::default(),
            extensions: ExtensionsConfig::default(),
            lobby: LobbySection::default(),
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LobbySection {
    pub default_mode: String,
}

impl Default for LobbySection {
    fn default() -> Self {
        LobbySection {
            default_mode: "grid".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("failed to parse config file {0}: {1}")]
    Parse(PathBuf, toml::de::Error),
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
        Ok(cfg)
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(port) = std::env::var("OXIPAGE_PORT")
            && let Ok(port) = port.parse::<u16>()
        {
            self.server.port = port;
        }
        if let Ok(dir) = std::env::var("OXIPAGE_DATA_DIR") {
            self.server.data_dir = PathBuf::from(dir);
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
data_dir = "/var/oxipage"

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
            std::path::PathBuf::from("/var/oxipage")
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
            std::env::set_var("OXIPAGE_PORT", "1234");
            std::env::set_var("OXIPAGE_DATA_DIR", "/tmp/oxi-test");
        }
        let mut cfg = Config::default();
        cfg.apply_env_overrides();
        assert_eq!(cfg.server.port, 1234);
        assert_eq!(
            cfg.server.data_dir,
            std::path::PathBuf::from("/tmp/oxi-test")
        );
        unsafe {
            std::env::remove_var("OXIPAGE_PORT");
            std::env::remove_var("OXIPAGE_DATA_DIR");
        }
    }
}

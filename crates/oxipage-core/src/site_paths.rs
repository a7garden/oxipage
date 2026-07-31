//! Runtime-mutable site settings (display, languages, lobby, integrations,
//! extensions, deploy). Server host/port/data_dir are intentionally excluded
//! — they are startup-immutable and captured once in `SiteContext::startup_server`.

use serde::{Deserialize, Serialize};

use crate::config::{
    Config, ExtensionsConfig, IntegrationsConfig, LobbySection, ServerConfig, SiteConfig,
};

/// Live-reloadable subset of site configuration. Excludes `[server]` fields
/// (host/port/data_dir) which are captured once at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableSiteSettings {
    pub site: MutableSiteConfig,
    pub lobby: MutableLobbyConfig,
    pub integrations: MutableIntegrationsConfig,
    pub extensions: MutableExtensionsConfig,
    #[serde(default)]
    pub deploy: DeployConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableSiteConfig {
    pub name: String,
    pub base_url: String,
    pub default_lang: String,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableLobbyConfig {
    pub default_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MutableIntegrationsConfig {
    #[serde(default)]
    pub github_username: Option<String>,
    #[serde(default)]
    pub tmdb_api_key_env: Option<String>,
    #[serde(default)]
    pub aladin_ttbkey_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MutableExtensionsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// Deploy target configuration. Consumed by the GitHub Pages deploy subproject.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployConfig {
    #[serde(default)]
    pub github_pages: Option<GitHubPagesTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPagesTarget {
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

impl MutableSiteSettings {
    /// Extract mutable settings from a full [`Config`].
    pub fn from_config(cfg: &Config) -> Self {
        MutableSiteSettings {
            site: MutableSiteConfig {
                name: cfg.site.name.clone(),
                base_url: cfg.site.base_url.clone(),
                default_lang: cfg.site.default_lang.clone(),
                languages: cfg.site.languages.clone(),
            },
            lobby: MutableLobbyConfig {
                default_mode: cfg.lobby.default_mode.clone(),
            },
            integrations: MutableIntegrationsConfig {
                github_username: cfg.integrations.github_username.clone(),
                tmdb_api_key_env: cfg.integrations.tmdb_api_key_env.clone(),
                aladin_ttbkey_env: cfg.integrations.aladin_ttbkey_env.clone(),
            },
            extensions: MutableExtensionsConfig {
                enabled: cfg.extensions.enabled.clone(),
            },
            deploy: DeployConfig::default(),
        }
    }

    /// Reconstruct a full [`Config`] from this snapshot plus the
    /// startup-immutable server section. Used for the legacy
    /// [`crate::state::AppState`] construction on the cold extension-enable
    /// path, where extensions still expect an `Arc<Config>`.
    pub fn to_config(&self, server: &ServerConfig) -> Config {
        Config {
            site: SiteConfig {
                name: self.site.name.clone(),
                base_url: self.site.base_url.clone(),
                default_lang: self.site.default_lang.clone(),
                languages: self.site.languages.clone(),
            },
            server: server.clone(),
            extensions: ExtensionsConfig {
                enabled: self.extensions.enabled.clone(),
            },
            integrations: IntegrationsConfig {
                github_username: self.integrations.github_username.clone(),
                tmdb_api_key_env: self.integrations.tmdb_api_key_env.clone(),
                aladin_ttbkey_env: self.integrations.aladin_ttbkey_env.clone(),
            },
            lobby: LobbySection {
                default_mode: self.lobby.default_mode.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn from_config_then_to_config_round_trips_mutable_fields() {
        let mut cfg = Config::default();
        cfg.site.name = "Round".into();
        cfg.site.base_url = "https://example.test".into();
        cfg.site.default_lang = "en".into();
        cfg.site.languages = vec!["en".into(), "ko".into()];
        cfg.lobby.default_mode = "list".into();
        cfg.extensions.enabled = vec!["blog".into()];
        cfg.integrations.github_username = Some("octocat".into());
        cfg.integrations.tmdb_api_key_env = Some("OXIPAGE_TMDB_KEY".into());
        cfg.integrations.aladin_ttbkey_env = Some("OXIPAGE_ALADIN_TTBKEY".into());

        let server = cfg.server.clone();
        let settings = MutableSiteSettings::from_config(&cfg);
        let rebuilt = settings.to_config(&server);

        // Mutable fields are preserved verbatim.
        assert_eq!(rebuilt.site.name, "Round");
        assert_eq!(rebuilt.site.base_url, "https://example.test");
        assert_eq!(rebuilt.site.default_lang, "en");
        assert_eq!(rebuilt.site.languages, vec!["en", "ko"]);
        assert_eq!(rebuilt.lobby.default_mode, "list");
        assert_eq!(rebuilt.extensions.enabled, vec!["blog"]);
        assert_eq!(rebuilt.integrations.github_username.as_deref(), Some("octocat"));
        assert_eq!(rebuilt.integrations.tmdb_api_key_env.as_deref(), Some("OXIPAGE_TMDB_KEY"));
        assert_eq!(
            rebuilt.integrations.aladin_ttbkey_env.as_deref(),
            Some("OXIPAGE_ALADIN_TTBKEY")
        );
        // Server section is passed through unchanged.
        assert_eq!(rebuilt.server.host, cfg.server.host);
        assert_eq!(rebuilt.server.port, cfg.server.port);
    }
}

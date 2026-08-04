use crate::model::GithubEvent;
use anyhow::Context;

const EVENTS_API: &str = "https://api.github.com";

pub struct GithubClient {
    http: reqwest::Client,
    username: Option<String>,
    base_url: String,
}

impl GithubClient {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::new(env_username(), EVENTS_API)
    }

    pub fn with_username(username: Option<String>) -> anyhow::Result<Self> {
        Self::new(username.or_else(env_username), EVENTS_API)
    }

    pub fn new(username: Option<String>, base_url: impl Into<String>) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "oxibuilder-ext-activity/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("failed to build GitHub HTTP client")?;
        Ok(Self {
            http,
            username,
            base_url: base_url.into(),
        })
    }

    pub fn enabled(&self) -> bool {
        self.username.is_some()
    }

    pub async fn fetch_public_events(&self) -> anyhow::Result<Vec<GithubEvent>> {
        let username = self
            .username
            .as_deref()
            .context("GitHub username is not configured")?;
        let url = format!(
            "{}/users/{username}/events/public",
            self.base_url.trim_end_matches('/')
        );
        self.http
            .get(url)
            .send()
            .await
            .context("GitHub Events API request failed")?
            .error_for_status()
            .context("GitHub Events API returned an error")?
            .json()
            .await
            .context("invalid GitHub Events API response")
    }
}
fn env_username() -> Option<String> {
    std::env::var("OXIBUILDER_GITHUB_USERNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

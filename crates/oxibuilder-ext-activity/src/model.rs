use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ActivityEvent {
    pub id: i64,
    pub repo_full_name: String,
    pub event_type: String,
    pub summary: String,
    pub url: String,
    pub occurred_at: String,
    pub synced_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ActivityEventInput {
    pub repo_full_name: String,
    pub event_type: String,
    pub summary: String,
    pub url: String,
    pub occurred_at: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub repo: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GithubEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub repo: GithubRepo,
    pub created_at: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GithubRepo {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

impl GithubEvent {
    pub fn into_input(self) -> ActivityEventInput {
        let event_type = normalize_event_type(&self.kind);
        let url = event_url(&self.payload)
            .or_else(|| self.repo.url.as_deref().map(api_repo_url_to_html))
            .unwrap_or_else(|| format!("https://github.com/{}", self.repo.name));
        ActivityEventInput {
            summary: summary(&event_type, &self.repo.name),
            repo_full_name: self.repo.name,
            event_type,
            url,
            occurred_at: self.created_at,
        }
    }
}

fn normalize_event_type(kind: &str) -> String {
    kind.strip_suffix("Event")
        .unwrap_or(kind)
        .chars()
        .enumerate()
        .flat_map(|(index, c)| {
            if index > 0 && c.is_ascii_uppercase() {
                vec!['_', c.to_ascii_lowercase()]
            } else {
                vec![c.to_ascii_lowercase()]
            }
        })
        .collect()
}

fn summary(event_type: &str, repo: &str) -> String {
    let action = match event_type {
        "push" => "push to",
        "pull_request" => "pull request on",
        "release" => "release on",
        "issues" => "issue on",
        "watch" | "star" => "starred",
        _ => event_type,
    };
    format!("{action} {repo}")
}

fn event_url(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("html_url")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            payload
                .pointer("/pull_request/html_url")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            payload
                .pointer("/release/html_url")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            payload
                .pointer("/issue/html_url")
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_owned)
}

fn api_repo_url_to_html(url: &str) -> String {
    url.replacen("https://api.github.com/repos/", "https://github.com/", 1)
}

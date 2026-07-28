use crate::client::Client;
use crate::output::Output;
use clap::Args;

#[derive(Args, Debug)]
pub struct CacheArgs {
    /// Extension to refresh (omit for all)
    #[arg(long)]
    pub extension: Option<String>,
}

pub(crate) async fn cache(
    c: CacheArgs,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    let path = if let Some(ref ext) = c.extension {
        format!("/api/v1/cache/refresh?extension={}", ext)
    } else {
        "/api/v1/cache/refresh".to_string()
    };

    let res = client.post(&path, &serde_json::json!({})).await?;
    out.data(res, "cache refresh")
}

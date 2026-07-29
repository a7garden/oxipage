use crate::client::Client;
use crate::output::Output;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct CacheArgs {
    /// Extension to refresh (omit for all)
    #[arg(long)]
    pub extension: Option<String>,
}

pub(crate) async fn cache(c: CacheArgs, out: &Output, client: &Client) -> anyhow::Result<()> {
    let path = if let Some(ref ext) = c.extension {
        format!("/api/v1/cache/refresh?extension={}", ext)
    } else {
        "/api/v1/cache/refresh".to_string()
    };

    match client.post(&path, &serde_json::json!({})).await {
        Ok(res) => out.data(res, "cache refresh"),
        Err(e) => {
            // Graceful fallback if server isn't running
            anyhow::bail!(
                "Cache refresh failed: {}. Make sure `oxipage console` is running.\n\
                 Alternatively, run `oxipage build` to generate the site with existing data.",
                e
            )
        }
    }
}

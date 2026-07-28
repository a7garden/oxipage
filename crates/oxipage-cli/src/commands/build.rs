use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand, Debug)]
pub enum BuildCommand {
    /// Build the static site from the DB.
    /// POST /api/v1/build → generates out/ directory.
    #[command(name = "build")]
    Run,
}

pub(crate) async fn build(
    c: BuildCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        BuildCommand::Run => {
            let res = client.post("/api/v1/build", &json!({})).await?;
            out.data(res, "build")
        }
    }
}

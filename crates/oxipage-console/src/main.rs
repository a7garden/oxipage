use oxipage_console::run_console;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_console().await
}

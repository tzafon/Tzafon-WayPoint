use std::time::Duration;

use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    loop {
        let stats = metrics_library::read_stats().ok_or(anyhow::anyhow!("Failed to read stats"))?;
        info!("Stats: {:?}", stats);
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

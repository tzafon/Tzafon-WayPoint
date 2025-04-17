use std::{path::Path, process::Stdio};

use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use instance_container::spawn_pipe_monitor;

/// Launches a headless Tzafonwright instance with the provided parameters
/// Starts the process and sets up monitoring tasks for stdout/stderr
pub async fn start_tzafonwright(
    tzafonwright_folder: &Path,
    cdp_url: &str,
    port: u16,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    let mut tzafonwright = tokio::process::Command::new("uv");
    // Set working directory
    tzafonwright
        .current_dir(tzafonwright_folder)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    // Ensure Tzafonw process is terminated when this process exits
    tzafonwright.kill_on_drop(true);

    tzafonwright
        .arg("run")
        .arg("src/tzafonwright/server.py")
        .arg("--port")
        .arg(port.to_string())
        .arg("--cdp-url")
        .arg(cdp_url);

    info!("Starting Tzafonwright {:?}", tzafonwright);
    let mut child = tzafonwright.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stderr"))?;

    tokio::spawn(async move {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                let res = child.kill().await;
                error!("Tzafonwright killed: {res:?}");
            }
            res = child.wait() => {
                error!("Tzafonwright exited: {res:?}");
                cancellation_token.cancel();
            }
        }
    });

    spawn_pipe_monitor(stderr, "Tzafonwright stderr");
    spawn_pipe_monitor(stdout, "Tzafonwright stdout");

    Ok(())
}

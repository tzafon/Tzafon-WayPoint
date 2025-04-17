use clap::Parser;
use shared::instance_manager::{InstanceId, InstanceType, Services};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;

/// Spawns a task that reads lines from a pipe and logs them with the given prefix
pub fn spawn_pipe_monitor(
    pipe: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    prefix: &str,
) -> JoinHandle<anyhow::Result<()>> {
    let prefix = prefix.to_string();
    tokio::spawn(async move {
        let mut reader = BufReader::new(pipe).lines();
        while let Some(line) = reader
            .next_line()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read from pipe: {e:?}"))?
        {
            debug!("{prefix}: {line}");
        }
        Ok::<(), anyhow::Error>(())
    })
}

// Get the IP address of the container
pub async fn get_ip_address() -> anyhow::Result<String> {
    use std::process::Command;
    let output = Command::new("hostname").arg("-i").output()?;

    anyhow::ensure!(
        output.status.success(),
        "Failed to get IP address: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Parser, Debug)]
pub struct SharedArgs {
    /// Log level
    #[clap(long, default_value_t = false)]
    pub debug_log: bool,
    #[clap(flatten)]
    pub instance_manager_config: instance_manager::ClientArgs,
    /// IP address to use for the instance
    #[clap(long)]
    pub ip_address: Option<String>,
}

pub fn create_services_from_args(
    ip_address: &String,
    cdp_port: Option<u16>,
    tzafonwright_port: Option<u16>,
    _ssh_port: Option<u16>,
) -> Services {
    Services {
        timestamp_ms: None,
        chrome_debug_port_service: cdp_port.map(|port| format!("{}:{}", ip_address, port)),
        tzafonwright_service: tzafonwright_port.map(|port| format!("{}:{}", ip_address, port)),
        // TODO: Add ssh service
        // ssh_service: ssh_port.map(|port| format!("{}:{}", ip_address, port)),
    }
}
pub async fn instance_manager_connection(
    instance_manager_config: &instance_manager::ClientArgs,
    instance_id: &InstanceId,
    instance_type: &InstanceType,
    services: Services,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    use shared::{metrics::start_system_metrics_loop, utils::start_health_loop};
    let channel = instance_manager::get_channel(instance_manager_config).await?;
    start_health_loop(
        instance_id,
        instance_type,
        &Some(services),
        &channel,
        cancellation_token,
    )
    .await?;

    start_system_metrics_loop(instance_id, &channel, cancellation_token).await?;
    Ok(())
}

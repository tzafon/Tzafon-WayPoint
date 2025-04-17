use std::time::Duration;

use sysinfo::System;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tonic::Request;
use tonic::transport::Channel;
use tracing::{info, warn};

use crate::instance_manager::post_service_client::PostServiceClient;
use crate::instance_manager::{InstanceDescription, InstanceId, SystemMetrics};
use crate::{add_version, get_timestamp_ms};

const METRICS_SLEEP: Duration = Duration::from_millis(5_000);

pub fn read_stats() -> Option<SystemMetrics> {
    let mut system = System::new();
    system.refresh_all();
    system.cgroup_limits().map(|limits| SystemMetrics {
        timestamp_ms: Some(get_timestamp_ms()),
        used_memory_bytes: limits.total_memory - limits.free_memory,
        total_memory_bytes: limits.total_memory,
    })
}

pub async fn start_system_metrics_loop(
    instance_id: &InstanceId,
    channel: &Channel,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    let mut client = PostServiceClient::with_interceptor(channel.clone(), add_version);
    let mut next_heartbeat = Instant::now();
    let instance_id = instance_id.clone();
    let cancellation_token = cancellation_token.clone();
    tokio::spawn(async move {
        loop {
            if let Some(stats) = tokio::task::spawn_blocking(read_stats).await? {
                let request = Request::new(InstanceDescription {
                    instance_id: Some(instance_id.clone()),
                    system_metrics: Some(stats),
                    ..Default::default()
                });
                if let Err(e) = client.post_instance_description(request).await {
                    warn!("Failed to post system metrics: {}", e);
                }
            } else {
                warn!("Failed to get system metrics");
            }
            next_heartbeat += METRICS_SLEEP;
            tokio::select! {
            _ = cancellation_token.cancelled() => {
                info!("Cancellation token cancelled");
                break;
            }
                _ = tokio::time::sleep_until(next_heartbeat) => {}
            }
        }
        Ok::<(), anyhow::Error>(())
    });
    Ok(())
}

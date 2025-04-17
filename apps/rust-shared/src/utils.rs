use std::time::Duration;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tonic::service::interceptor::InterceptedService;
use tonic::{Request, Status, transport::Channel};
use tracing::{error, info};

use crate::add_version;
use crate::instance_manager::try_service_client::TryServiceClient;
use crate::instance_manager::{
    HealthCheck, InstanceDescription, InstanceId, InstanceType, Services,
};

const HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(1_000);
const MAX_RETRIES: usize = 3;

type Client =
    TryServiceClient<InterceptedService<Channel, fn(Request<()>) -> Result<Request<()>, Status>>>;

fn start_heart_beat(
    instance_id: &InstanceId,
    mut client: Client,
    cancellation_token: &CancellationToken,
) {
    let instance_id = instance_id.clone();
    let cancellation_token = cancellation_token.clone();
    tokio::spawn(async move {
        let mut next_heart_beat = Instant::now();
        let mut retries = 0;
        loop {
            let request = Request::new(InstanceDescription {
                instance_id: Some(instance_id.clone()),
                health_check: Some(HealthCheck { timestamp_ms: None }),
                ..Default::default()
            });
            let response = client
                .try_update_instance_description(request)
                .await
                .map(|v| v.into_inner().value);
            match (response, retries) {
                (Ok(true), _) => {
                    retries = 0;
                }
                (Ok(false), _) | (_, MAX_RETRIES) => {
                    error!("Instance is unhealthy should be killed");
                    break;
                }
                (Err(e), _) => {
                    error!("Error updating instance description: {}", e);
                    retries += 1;
                }
            }

            next_heart_beat += HEALTH_CHECK_INTERVAL;

            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    error!("Heartbeat cancelled");
                    break;
                }
                _ = tokio::time::sleep_until(next_heart_beat) => {
                    continue;
                }
            }
        }
        cancellation_token.cancel();
    });
}

pub async fn initialize_health_loop(
    instance_id: &InstanceId,
    instance_type: &InstanceType,
    services: &Option<Services>,
    client: &mut Client,
) -> anyhow::Result<()> {
    match client
        .try_add_instance(Request::new(InstanceDescription {
            instance_id: Some(instance_id.clone()),
            instance_type: Some(*instance_type as i32),
            services: services.clone(),
            ..Default::default()
        }))
        .await
        .map(|v| v.into_inner().value)
    {
        Ok(true) => {
            info!("Instance added successfully");
        }
        Ok(false) => {
            return Err(anyhow::anyhow!("Failed to add instance already exists"));
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Error adding instance: {}", e));
        }
    }
    Ok(())
}
pub async fn start_health_loop(
    instance_id: &InstanceId,
    instance_type: &InstanceType,
    services: &Option<Services>,
    channel: &Channel,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    let mut client: Client = TryServiceClient::with_interceptor(channel.clone(), add_version);
    if let Err(e) = initialize_health_loop(instance_id, instance_type, services, &mut client).await
    {
        error!("Failed to initialize health loop: {:?}", e);
        return Err(e);
    }
    start_heart_beat(instance_id, client, cancellation_token);
    Ok(())
}

pub fn generate_instance_id(prefix: &str) -> String {
    format!("{}-{}", prefix, uuid::Uuid::new_v4())
}

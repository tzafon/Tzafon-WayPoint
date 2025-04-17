use clap::Parser;
use tokio_util::sync::CancellationToken;
use tonic::{Request, transport::Channel};
use tracing::{error, info};

use shared::instance_manager::get_service_client::GetServiceClient;
use shared::instance_manager::try_service_client::TryServiceClient;
use shared::instance_manager::{
    AllInstancesQuery, InstanceType, KillInstanceRequest, KillReason, Relationship,
};
use shared::instance_manager::{HealthCheck, InstanceDescription, InstanceId, TimestampMs};
use shared::socket_gateway::http_proxy::{
    HttpProxyConfigTrait, HttpProxyInstance, ServerConnectionManagerTrait,
};
use shared::socket_gateway::simple_gateway::{
    HttpProxyConfig, PathOverride, start_simple_http_gateway_with_proxy_config,
};
use shared::{add_version, get_timestamp_ms};

const INSTANCE_ID_PREFIX: &str = "ephemeral-browser-proxy";

#[derive(Parser, Debug)]
struct Args {
    /// Port to accept connections on
    #[clap(long, default_value_t = 9222)]
    cdp_port: u16,
    /// Tzafonwright port
    #[clap(long, default_value_t = 1337)]
    tzafonwright_port: u16,
    /// Log level
    #[clap(long, default_value_t = false)]
    debug_log: bool,
    /// Hostname for metrics
    #[clap(long, env = "HOSTNAME")]
    instance_id: Option<String>,
    #[clap(flatten)]
    instance_manager: instance_manager::ClientArgs,
}
#[allow(clippy::upper_case_acronyms)]
enum ProxyType {
    CDP,
    TZAFONWRIGHT,
}
struct ChromeWarmpoolProxyConfig {
    channel: Channel,
    instance_id: InstanceId,
    proxy_type: ProxyType,
}

struct ServerConnectionManager {
    instance_id: String,
    channel: Channel,
}

impl ServerConnectionManagerTrait for ServerConnectionManager {
    async fn on_open(&mut self) -> Result<(), shared::socket_gateway::http_proxy::Error> {
        info!("Connected to instance: {}", self.instance_id);
        Ok(())
    }

    async fn on_close(
        &mut self,
        close_result: Result<(), shared::socket_gateway::http_proxy::Error>,
    ) -> Result<(), shared::socket_gateway::http_proxy::Error> {
        info!("Disconnected from instance: {}", self.instance_id);
        if let Err(e) = close_result {
            error!("Error in on_close: {:?}", e);
        }
        let mut service_client =
            TryServiceClient::with_interceptor(self.channel.clone(), add_version);
        service_client
            .try_update_instance_description(Request::new(InstanceDescription {
                instance_id: Some(InstanceId {
                    instance_id: self.instance_id.clone(),
                }),
                kill_instance_request: Some(KillInstanceRequest {
                    kill_reason: KillReason::Killed as i32,
                    timestamp_ms: None,
                }),
                ..Default::default()
            }))
            .await
            .map_err(|_| {
                shared::socket_gateway::http_proxy::Error::IoError("Failed to kill instance")
            })?;
        Ok(())
    }
}

impl ChromeWarmpoolProxyConfig {
    async fn get_instance(
        &self,
    ) -> Result<InstanceDescription, shared::socket_gateway::http_proxy::Error> {
        let mut client = GetServiceClient::with_interceptor(self.channel.clone(), add_version);
        let mut interaction_client =
            TryServiceClient::with_interceptor(self.channel.clone(), add_version);
        for instance in client
            .get_all_instances(Request::new(AllInstancesQuery {
                instance_type: InstanceType::ChromeBrowser as i32,
            }))
            .await
            .map_err(|_| {
                shared::socket_gateway::http_proxy::Error::IoError("Failed to get instances")
            })?
            .into_inner()
            .instance_ids
        {
            let instance_description = if let Ok(instance_health) = client
                .get_instance(Request::new(InstanceId {
                    instance_id: instance.instance_id.clone(),
                }))
                .await
            {
                instance_health.into_inner()
            } else {
                continue;
            };
            info!("Instance description: {:?}", instance_description);
            match &instance_description {
                InstanceDescription {
                    health_check:
                        Some(HealthCheck {
                            timestamp_ms: Some(TimestampMs { timestamp_ms }),
                            ..
                        }),
                    parent: None,
                    kill_instance_request: None,
                    ..
                } if *timestamp_ms > (get_timestamp_ms().timestamp_ms - 1000 * 5) as u64 => {
                    info!("Found healthy instance");
                    let res = interaction_client
                        .try_update_instance_description(Request::new(InstanceDescription {
                            instance_id: Some(InstanceId {
                                instance_id: instance.instance_id.clone(),
                            }),
                            parent: Some(Relationship {
                                instance_id: Some(self.instance_id.clone()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                        .await;
                    info!("Update instance description response: {:?}", res);
                    if let Ok(response) = res {
                        if response.into_inner().value {
                            return Ok(instance_description);
                        }
                    }
                }
                _ => continue,
            };
        }
        Err(shared::socket_gateway::http_proxy::Error::IoError(
            "No available instance found",
        ))
    }
    async fn get_proxy_config(
        &self,
        instance_description: InstanceDescription,
    ) -> Result<HttpProxyConfig, shared::socket_gateway::http_proxy::Error> {
        let address = instance_description
            .services
            .and_then(|service| match self.proxy_type {
                ProxyType::CDP => service.chrome_debug_port_service,
                ProxyType::TZAFONWRIGHT => service.tzafonwright_service,
            })
            .ok_or(shared::socket_gateway::http_proxy::Error::IoError(
                "Instance has no address",
            ))?;
        let proxy_config = HttpProxyConfig::new(&address)
            .with_path_override(PathOverride::Replace("/".to_string()))
            .with_header_override("Host", &address);

        Ok(proxy_config)
    }
}

impl HttpProxyConfigTrait<ServerConnectionManager> for ChromeWarmpoolProxyConfig {
    async fn new_connection(
        &mut self,
        request: shared::socket_gateway::http_proxy::Request,
    ) -> Result<
        shared::socket_gateway::http_proxy::HttpProxyInstance<ServerConnectionManager>,
        shared::socket_gateway::http_proxy::Error,
    > {
        let instance_description = self.get_instance().await?;
        let instance_id = instance_description
            .clone()
            .instance_id
            .ok_or(shared::socket_gateway::http_proxy::Error::IoError(
                "Instance has no id",
            ))?
            .instance_id;
        let proxy_config = self.get_proxy_config(instance_description).await?;
        let request = proxy_config.modify_request(request).await?;

        Ok(HttpProxyInstance {
            request,
            server: tokio::net::TcpStream::connect(&proxy_config.server_addr)
                .await
                .map_err(|_| {
                    shared::socket_gateway::http_proxy::Error::IoError(
                        "Failed to connect to instance",
                    )
                })?,
            manager: ServerConnectionManager {
                instance_id,
                channel: self.channel.clone(),
            },
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::try_parse()?;
    tracing_subscriber::fmt()
        // .with_max_level(Level::TRACE)
        .init();
    let instance_id = InstanceId {
        instance_id: shared::utils::generate_instance_id(INSTANCE_ID_PREFIX),
    };

    let channel = instance_manager::get_channel(&args.instance_manager).await?;

    let cancellation_token = CancellationToken::new();
    shared::utils::start_health_loop(
        &instance_id,
        &InstanceType::WarmpoolChromeProxy,
        &None,
        &channel,
        &cancellation_token,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to start health loop: {:?}", e))?;

    start_simple_http_gateway_with_proxy_config(
        ChromeWarmpoolProxyConfig {
            channel: channel.clone(),
            instance_id: instance_id.clone(),
            proxy_type: ProxyType::CDP,
        },
        format!("0.0.0.0:{}", args.cdp_port)
            .parse()
            .map_err(|_| anyhow::anyhow!("Failed to parse listen address"))?,
        &cancellation_token,
    )
    .await?;
    start_simple_http_gateway_with_proxy_config(
        ChromeWarmpoolProxyConfig {
            channel: channel.clone(),
            instance_id: instance_id.clone(),
            proxy_type: ProxyType::TZAFONWRIGHT,
        },
        format!("0.0.0.0:{}", args.tzafonwright_port)
            .parse()
            .map_err(|_| anyhow::anyhow!("Failed to parse listen address"))?,
        &cancellation_token,
    )
    .await?;

    cancellation_token.cancelled().await;
    Ok(())
}

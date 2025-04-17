mod chrome;
mod tzafonwright;

use anyhow::Context;
use std::path::PathBuf;

use instance_container::{SharedArgs, get_ip_address, create_services_from_args, instance_manager_connection};

use clap::Parser;
use shared::socket_gateway::simple_gateway::{
    HttpProxyConfig, PathOverride, start_simple_http_gateway_with_proxy_config,
};
use tokio_util::sync::CancellationToken;
use tracing::info;

use shared::instance_manager::{InstanceId, InstanceType};

const INSTANCE_ID_PREFIX: &str = "browser-container";

#[derive(Parser, Debug)]
struct Args {
    /// Path to the Chrome binary
    #[clap(long)]
    chrome_binary_path: String,
    /// Port to accept connections on
    #[clap(long, default_value_t = 9222)]
    cdp_port: u16,
    /// Tzafonwright port
    #[clap(long, default_value_t = 1337)]
    tzafonwright_port: u16,
    /// Path to the Tzafonwright binary
    #[clap(long, default_value = "/app/tzafonwright")]
    tzafonwright_binary_path: PathBuf,
    #[clap(flatten)]
    shared_args: SharedArgs,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cancellation_token = CancellationToken::new();
    let args = Args::try_parse()?;

    tracing_subscriber::fmt()
        .with_max_level(if args.shared_args.debug_log {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

    let instance_id = InstanceId {
        instance_id: shared::utils::generate_instance_id(INSTANCE_ID_PREFIX),
    };
    info!("Instance ID: {:?}", instance_id);

    let ip_address = if let Some(ip_address) = &args.shared_args.ip_address {
        ip_address.to_owned()
    } else {
        get_ip_address()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get IP address: {}", e))?
    };
    info!("IP address: {}", ip_address);

    info!("Starting browser container with args: {:?}", args);
    let ws_path =
        chrome::start_chrome(&args.chrome_binary_path, cancellation_token.clone()).await?;

    info!("Chrome started, internal ws path: {}", ws_path);
    tzafonwright::start_tzafonwright(
        &args.tzafonwright_binary_path,
        &ws_path,
        args.tzafonwright_port,
        cancellation_token.clone(),
    )
    .await?;

    info!("Tzafonwright started");

    let uri = hyper::Uri::from_maybe_shared(ws_path)?;
    let (host, port) = uri
        .host()
        .zip(uri.port())
        .ok_or(anyhow::anyhow!("No host or port"))?;
    let server_addr = format!("{}:{}", host, port);

    let proxy_config = HttpProxyConfig::new(&server_addr)
        .with_header_override("Host", &server_addr)
        .with_path_override(PathOverride::Replace(uri.path().to_string()));

    let listen_addr = format!("0.0.0.0:{}", args.cdp_port).parse()?;

    start_simple_http_gateway_with_proxy_config(proxy_config, listen_addr, &cancellation_token)
        .await
        .context("Failed to start gateway")?;

    info!("Proxy started");
    let services = create_services_from_args(
        &ip_address,
        Some(args.cdp_port),
        Some(args.tzafonwright_port),
        None,
    );
    instance_manager_connection(
        &args.shared_args.instance_manager_config,
        &instance_id,
        &InstanceType::ChromeBrowser,
        services,
        &cancellation_token,
    )
    .await
    .context("Failed to start instance manager connection")?;

    cancellation_token.cancelled().await;
    info!("Program exiting");
    Ok(())
}

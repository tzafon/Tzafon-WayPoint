mod service;
pub(crate) mod status_page;
mod traits;

use clap::Parser;
use tracing::info;

use service::Service;

use shared::instance_manager::get_service_server::GetServiceServer;
use shared::instance_manager::post_service_server::PostServiceServer;
use shared::instance_manager::try_service_server::TryServiceServer;
use shared::{PROTO_VERSION, check_version};

use instance_manager::{ServerArgs, get_server};

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(long, default_value_t = false)]
    debug_log: bool,
    #[clap(long, default_value_t = 4242)]
    status_page_port: u16,
    #[clap(flatten)]
    server_args: ServerArgs,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = format!("0.0.0.0:{}", args.server_args.port).parse()?;
    tracing_subscriber::fmt()
        .with_max_level(if args.debug_log {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .with_target(false)
        .init();
    info!(
        "Listening on {}, using proto_version={}",
        addr, PROTO_VERSION
    );
    let service = Service::new();
    service.clone().start_kill_loop().await;
    service
        .clone()
        .start_status_page(args.status_page_port)
        .await?;
    get_server(&args.server_args)?
        .add_service(TryServiceServer::with_interceptor(
            service.clone(),
            check_version,
        ))
        .add_service(PostServiceServer::with_interceptor(
            service.clone(),
            check_version,
        ))
        .add_service(GetServiceServer::with_interceptor(
            service.clone(),
            check_version,
        ))
        .serve(addr)
        .await?;

    Ok(())
}

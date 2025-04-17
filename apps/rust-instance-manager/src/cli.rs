use std::collections::HashMap;

use clap::Parser;
use tonic::Request;
use tracing::info;

use instance_manager::{ClientArgs, get_channel};

use shared::{
    add_version,
    instance_manager::{AllInstancesQuery, InstanceType, get_service_client},
};

fn parse_instance_type(s: &str) -> anyhow::Result<InstanceType> {
    InstanceType::from_str_name(s).ok_or(anyhow::anyhow!("Invalid instance type: {}", s))
}

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(long, value_parser = parse_instance_type)]
    instance_type: InstanceType,
    #[clap(long)]
    pub has_parent: bool,
    #[clap(long)]
    pub alive: bool,
    #[clap(flatten)]
    client_args: ClientArgs,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::try_parse()?;
    tracing_subscriber::fmt::init();

    let channel = get_channel(&args.client_args).await?;

    let mut client = get_service_client::GetServiceClient::with_interceptor(channel, add_version);

    let instance_ids = client
        .get_all_instances(Request::new(AllInstancesQuery {
            instance_type: args.instance_type as i32,
        }))
        .await?
        .into_inner()
        .instance_ids;

    let instances = instance_ids
        .into_iter()
        .map(|instance_id| {
            let mut client = client.clone();
            tokio::spawn(
                async move { client.get_instance(Request::new(instance_id.clone())).await },
            )
        })
        .collect::<Vec<_>>();
    info!(
        "{} instances of type {:?} found",
        instances.len(),
        args.instance_type
    );
    let mut num_children = HashMap::new();
    for instance in instances {
        match instance.await {
            Ok(Ok(instance)) => {
                let instance = instance.into_inner();
                let has_parent = instance.parent.is_some();
                if args.has_parent != has_parent {
                    continue;
                }
                let is_alive =
                    instance.health_check.is_some() && instance.kill_instance_request.is_none();
                if args.alive != is_alive {
                    continue;
                }
                let key = instance
                    .parent
                    .and_then(|p| p.instance_id)
                    .map(|id| id.instance_id)
                    .ok_or(anyhow::anyhow!("No parent instance id found"))?;
                *num_children.entry(key).or_insert(0) += 1;
            }
            e => {
                info!("Error: {:?}", e);
            }
        }
    }
    info!("Num children of parent map: {:?}", num_children);
    Ok(())
}

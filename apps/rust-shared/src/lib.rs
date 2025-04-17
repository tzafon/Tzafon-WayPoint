pub mod instance_manager {
    tonic::include_proto!("instance_manager");
}
pub mod metrics;
pub mod socket_gateway;
pub mod utils;

use instance_manager::TimestampMs;
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::metadata::MetadataValue;
use tonic::{Request, Status};

pub const PROTO_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/proto_version"));

pub fn check_version(req: Request<()>) -> Result<Request<()>, Status> {
    match req.metadata().get("proto_version") {
        Some(t) if PROTO_VERSION == t => Ok(req),
        Some(_) => Err(Status::failed_precondition("Wrong protocol versions")),
        _ => Err(Status::failed_precondition("No version supplied")),
    }
}

pub fn add_version(mut req: Request<()>) -> Result<Request<()>, Status> {
    #[allow(clippy::unwrap_used)]
    let proto_version: MetadataValue<_> = PROTO_VERSION.parse().unwrap();
    req.metadata_mut().insert("proto_version", proto_version);
    Ok(req)
}

pub fn get_timestamp_ms() -> TimestampMs {
    TimestampMs {
        #[allow(clippy::unwrap_used)]
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    }
}

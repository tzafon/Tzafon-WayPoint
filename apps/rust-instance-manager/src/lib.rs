use std::path::PathBuf;

use anyhow::Context;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity, ServerTlsConfig};

#[derive(Debug, clap::Parser)]
pub struct ClientArgs {
    #[clap(long)]
    pub instance_manager: String,
    #[clap(long, default_value = "/etc/ssl_certs/ca/tls.crt")]
    pub ca_path: PathBuf,
    #[clap(long, default_value = "/etc/ssl_certs/client/tls.crt")]
    pub cert_path: PathBuf,
    #[clap(long, default_value = "/etc/ssl_certs/client/tls.key")]
    pub key_path: PathBuf,
}

#[derive(Debug, clap::Parser)]
pub struct ServerArgs {
    #[clap(long, default_value_t = 50052)]
    pub port: u16,
    #[clap(long, default_value = "/etc/ssl_certs/ca/tls.crt")]
    pub ca_path: PathBuf,
    #[clap(long, default_value = "/etc/ssl_certs/server/tls.crt")]
    pub cert_path: PathBuf,
    #[clap(long, default_value = "/etc/ssl_certs/server/tls.key")]
    pub key_path: PathBuf,
}

fn get_client_tls_config(args: &ClientArgs) -> anyhow::Result<ClientTlsConfig> {
    Ok(ClientTlsConfig::new()
        .identity(Identity::from_pem(
            std::fs::read_to_string(&args.cert_path).context("Failed to read cert file")?,
            std::fs::read_to_string(&args.key_path).context("Failed to read key file")?,
        ))
        .ca_certificate(Certificate::from_pem(
            std::fs::read_to_string(&args.ca_path).context("Failed to read ca file")?,
        )))
}

fn get_server_tls_config(args: &ServerArgs) -> anyhow::Result<ServerTlsConfig> {
    Ok(ServerTlsConfig::new()
        .identity(Identity::from_pem(
            std::fs::read_to_string(&args.cert_path)?,
            std::fs::read_to_string(&args.key_path)?,
        ))
        .client_ca_root(Certificate::from_pem(std::fs::read_to_string(
            &args.ca_path,
        )?)))
}

pub fn get_server(args: &ServerArgs) -> anyhow::Result<tonic::transport::server::Server> {
    Ok(tonic::transport::server::Server::builder().tls_config(get_server_tls_config(args)?)?)
}

pub async fn get_channel(args: &ClientArgs) -> anyhow::Result<Channel> {
    let tls_config = get_client_tls_config(args)?;
    let channel = Channel::from_shared(args.instance_manager.clone())?
        .tls_config(tls_config)?
        .connect()
        .await?;
    Ok(channel)
}

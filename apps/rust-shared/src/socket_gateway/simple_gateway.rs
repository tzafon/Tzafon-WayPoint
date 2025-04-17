use std::{collections::HashMap, net::SocketAddr};

use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::socket_gateway::http_proxy::{
    Error, HttpProxyConfigTrait, HttpProxyInstance, Request, ServerConnectionManagerTrait,
    start_http_proxy_connection,
};

#[derive(Debug, Clone)]
pub enum PathOverride {
    /// Adds prefix to the path, probably no ending slash otherwise you will get something like "/PREFIX//PATH"
    /// If user path is "/" it is not added after
    /// e.g :
    /// - "/PREFIX/ROUTE" and "/USER/ROUTE" = "/PREFIX/ROUTE/USER/ROUTE"
    /// - "/PREFIX/ROUTE" and "/" = "/PREFIX/ROUTE"
    Prefix(String),
    /// Replaces the path
    /// e.g :
    /// - "/REPLACE/ROUTE" and "/USER/ROUTE" = "/REPLACE/ROUTE"
    Replace(String),
    /// Adds suffix to the path
    /// If user path is "/" it is not added before
    /// e.g :
    /// - "/SUFFIX/ROUTE" and "/USER/ROUTE" = "/USER/ROUTE/SUFFIX/ROUTE"
    /// - "/SUFFIX/ROUTE" and "/" = "/SUFFIX/ROUTE"
    Append(String),
}

#[derive(Debug)]
pub struct HttpProxyConfig {
    pub overide_headers: HashMap<String, String>,
    pub path_override: PathOverride,
    pub server_addr: String,
    pub connection_count: usize,
}
impl HttpProxyConfig {
    pub fn new(server_addr: &str) -> Self {
        Self {
            overide_headers: HashMap::new(),
            path_override: PathOverride::Prefix("/".to_string()),
            server_addr: server_addr.to_string(),
            connection_count: 0,
        }
    }
    pub fn with_path_override(mut self, path_override: PathOverride) -> Self {
        self.path_override = path_override;
        self
    }
    pub fn with_header_override(mut self, key: &str, value: &str) -> Self {
        self.overide_headers
            .insert(key.to_string(), value.to_string());
        self
    }
    pub async fn modify_request(&self, mut request: Request) -> Result<Request, Error> {
        request.path = match (&self.path_override, request.path.as_str()) {
            (PathOverride::Replace(path), _)
            | (PathOverride::Prefix(path), "/")
            | (PathOverride::Append(path), "/") => path.to_string(),
            (PathOverride::Prefix(prefix), path) => {
                format!("{}{}", prefix, path)
            }
            (PathOverride::Append(suffix), path) => {
                format!("{}{}", path, suffix)
            }
        };
        request
            .headers
            .retain(|(key, _)| !self.overide_headers.contains_key(key));
        request
            .headers
            .extend(self.overide_headers.clone().into_iter());
        Ok(request)
    }
}
pub struct ServerConnectionManager {
    connection_id: usize,
}
impl ServerConnectionManagerTrait for ServerConnectionManager {
    async fn on_open(&mut self) -> Result<(), Error> {
        info!("#{} Server connection opened", self.connection_id);
        Ok(())
    }

    async fn on_close(&mut self, close_result: Result<(), Error>) -> Result<(), Error> {
        info!("#{} Server connection closed", self.connection_id);
        if let Err(e) = close_result {
            error!("#{} Error on close: {:?}", self.connection_id, e);
        }
        Ok(())
    }
}

impl HttpProxyConfigTrait<ServerConnectionManager> for HttpProxyConfig {
    async fn new_connection(
        &mut self,
        request: Request,
    ) -> Result<HttpProxyInstance<ServerConnectionManager>, Error> {
        let request = self.modify_request(request).await?;
        let server = tokio::net::TcpStream::connect(&self.server_addr)
            .await
            .map_err(|_| Error::IoError("Failed to connect to server"))?;
        self.connection_count += 1;
        Ok(HttpProxyInstance {
            request,
            server,
            manager: ServerConnectionManager {
                connection_id: self.connection_count,
            },
        })
    }
}


pub async fn start_simple_gateway_with_full_address(
    server_addr: String,
    listen_addr: SocketAddr,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .map_err(|_| anyhow::anyhow!("Failed to start simple gateway: Failed to bind address"))?;
    let cancellation_token = cancellation_token.clone();
    tokio::spawn(async move {
        loop {
            if let Ok((client, _)) = tokio::select! {
                _ = cancellation_token.cancelled() => {
                    error!("Cancellation token cancelled");
                    break;
                }
                c = listener.accept() => {c}
            } {
                if let Err(e) = start_proxy_connection(&server_addr, client).await {
                    warn!("Failed to start proxy connection: {:?}", e);
                }
            } else {
                error!("Listener closed");
                break;
            }
        }
        cancellation_token.cancel();
    });
    Ok(())
}

pub async fn start_simple_gateway(
    server_host: &str,
    server_port: u16,
    listen_addr: SocketAddr,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    let server_addr = format!("{}:{}", server_host, server_port);
    start_simple_gateway_with_full_address(server_addr, listen_addr, cancellation_token).await
}

async fn start_proxy_connection(
    server_addr: &str,
    mut client: tokio::net::TcpStream,
) -> Result<(), Error> {
    let server_addr = server_addr.to_string();
    tokio::spawn(async move {
        let mut server = tokio::net::TcpStream::connect(server_addr)
            .await
            .map_err(|_| Error::IoError("Failed to connect to server"))?;
        let proxy_result = async {
            tokio::io::copy_bidirectional(&mut client, &mut server)
                .await
                .map_err(|_| Error::IoError("Failed while sending data to/from server"))?;
            Ok::<(), Error>(())
        }
        .await;

        info!("Proxy connection closed, with result: {:?}", proxy_result);

        // This executes when the connection terminates
        Ok::<(), Error>(())
    });
    Ok(())
}

pub async fn start_simple_http_gateway(
    server_host: &str,
    server_port: u16,
    listen_addr: SocketAddr,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    let server_addr = format!("{}:{}", server_host, server_port);
    let proxy_config =
        HttpProxyConfig::new(&server_addr).with_header_override("Host", &server_addr);
    start_simple_http_gateway_with_proxy_config(proxy_config, listen_addr, cancellation_token).await
}

pub async fn start_simple_http_gateway_with_proxy_config<
    T: ServerConnectionManagerTrait + 'static + Send + Sync,
    P: HttpProxyConfigTrait<T> + Send + Sync + 'static,
>(
    mut proxy_config: P,
    listen_addr: SocketAddr,
    cancellation_token: &CancellationToken,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .map_err(|_| anyhow::anyhow!("Failed to start simple gateway: Failed to bind address"))?;
    let cancellation_token = cancellation_token.clone();
    tokio::spawn(async move {
        loop {
            if let Ok((client, _)) = tokio::select! {
                _ = cancellation_token.cancelled() => {
                    error!("Cancellation token cancelled");
                    break;
                }
                c = listener.accept() => {c}
            } {
                if let Err(e) = start_http_proxy_connection(&mut proxy_config, client).await {
                    warn!("Failed to start proxy connection: {:?}", e);
                }
            } else {
                error!("Listener closed");
                break;
            }
        }
        cancellation_token.cancel();
    });
    Ok(())
}
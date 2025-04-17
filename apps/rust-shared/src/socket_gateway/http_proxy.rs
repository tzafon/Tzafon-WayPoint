use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::warn;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    ParseError(&'static str),
    IoError(&'static str),
}

async fn read_until_empty_line(
    stream: &mut tokio::net::TcpStream,
    data: &mut String,
) -> Result<(), Error> {
    data.clear();
    loop {
        data.push(
            stream
                .read_u8()
                .await
                .map_err(|_| Error::IoError("Failed to read"))? as char,
        );
        if data.ends_with("\r\n\r\n") {
            // Remove the newline characters and optional spaces
            return Ok(());
        }
    }
}

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
}
impl Request {
    pub fn new(data: &str) -> Result<Self, Error> {
        let mut lines = data.split("\r\n");

        let header_line = lines
            .next()
            .ok_or(Error::ParseError("Missing header line"))?;

        let (method, path, version) = Self::parse_request_line(header_line)?;
        let mut result = Self {
            method: method.to_string(),
            path: path.to_string(),
            version: version.to_string(),
            headers: Vec::new(),
        };
        for line in lines {
            if line.is_empty() {
                return Ok(result);
            }
            let (key, value) = Self::parse_header_line(line)?;
            result.headers.push((key.to_string(), value.to_string()));
        }
        Err(Error::ParseError("Missing empty line"))
    }

    fn parse_request_line(line: &str) -> Result<(&str, &str, &str), Error> {
        let mut parts = line.splitn(3, " ");
        let method = parts.next().ok_or(Error::ParseError("Missing method"))?;
        let path = parts.next().ok_or(Error::ParseError("Missing path"))?;
        let version = parts.next().ok_or(Error::ParseError("Missing version"))?;
        Ok((method, path, version))
    }

    fn parse_header_line(line: &str) -> Result<(&str, &str), Error> {
        let (key, value) = line
            .split_once(":")
            .ok_or(Error::ParseError("Malformed header line"))?;
        Ok((key.trim(), value.trim()))
    }

    async fn write_arr(line: &[&str], stream: &mut tokio::net::TcpStream) -> Result<(), Error> {
        for &l in line {
            stream
                .write_all(l.as_bytes())
                .await
                .map_err(|_| Error::IoError("Failed to write"))?;
        }
        Ok(())
    }
    async fn write_request_line(&self, stream: &mut tokio::net::TcpStream) -> Result<(), Error> {
        Self::write_arr(
            &[&self.method, " ", &self.path, " ", &self.version, "\r\n"],
            stream,
        )
        .await?;
        Ok(())
    }
    async fn write_header_line(
        key: &str,
        value: &str,
        stream: &mut tokio::net::TcpStream,
    ) -> Result<(), Error> {
        Self::write_arr(&[key, ": ", value, "\r\n"], stream).await?;
        Ok(())
    }
    async fn write_to_stream(self, stream: &mut tokio::net::TcpStream) -> Result<(), Error> {
        Self::write_request_line(&self, stream).await?;
        for (key, value) in &self.headers {
            Self::write_header_line(key, value, stream).await?;
        }
        Self::write_arr(&["\r\n"], stream).await?;
        Ok(())
    }
}

pub struct HttpProxyInstance<M: ServerConnectionManagerTrait> {
    pub request: Request,
    pub server: tokio::net::TcpStream,
    pub manager: M,
}

pub trait HttpProxyConfigTrait<M: ServerConnectionManagerTrait> {
    fn new_connection(
        &mut self,
        request: Request,
    ) -> impl std::future::Future<Output = Result<HttpProxyInstance<M>, Error>> + Send;
}

pub trait ServerConnectionManagerTrait: Send + Sync {
    fn on_open(&mut self) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn on_close(
        &mut self,
        close_result: Result<(), Error>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
}

pub async fn start_http_proxy_connection<
    C: HttpProxyConfigTrait<M>,
    M: ServerConnectionManagerTrait + 'static,
>(
    proxy_config: &mut C,
    mut client: tokio::net::TcpStream,
) -> Result<(), Error> {
    let mut data = String::with_capacity(1024);
    read_until_empty_line(&mut client, &mut data).await?;
    let request = Request::new(&data)?;
    let instance = proxy_config.new_connection(request).await?;
    tokio::spawn(async move {
        let HttpProxyInstance {
            request,
            mut server,
            mut manager,
        } = instance;
        let proxy_result = async {
            manager.on_open().await?;
            request.write_to_stream(&mut server).await?;
            tokio::io::copy_bidirectional(&mut client, &mut server)
                .await
                .map_err(|_| Error::IoError("Failed while sending data to/from server"))?;
            Ok::<(), Error>(())
        }
        .await;

        // This executes when the connection terminates
        if let Err(e) = manager.on_close(proxy_result).await {
            warn!("Error in on_close: {:?}", e);
        }
        Ok::<(), Error>(())
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request() {
        let request_str = "GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 10\r\n\r\n";
        #[allow(clippy::unwrap_used)]
        let request = Request::new(request_str).unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/");
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.headers.len(), 2);
        assert_eq!(request.headers[0].0, "Host");
        assert_eq!(request.headers[0].1, "example.com");
        assert_eq!(request.headers[1].0, "Content-Length");
        assert_eq!(request.headers[1].1, "10");
    }
}

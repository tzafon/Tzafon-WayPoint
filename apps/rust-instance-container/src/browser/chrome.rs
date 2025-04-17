use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use instance_container::spawn_pipe_monitor;

fn parse_url_from_line(line: &str) -> anyhow::Result<String> {
    Ok(line
        .split(' ')
        .last()
        .ok_or_else(|| anyhow::anyhow!("Failed to get ws path"))?
        .to_string())
}

/// Launches a headless Chrome instance with DevTools debugging enabled
/// Returns the WebSocket URL for DevTools connection
pub async fn start_chrome(
    chrome_binary_path: &str,
    cancellation_token: CancellationToken,
) -> anyhow::Result<String> {
    let mut chrome = tokio::process::Command::new(chrome_binary_path);
    chrome.stdout(Stdio::piped());
    chrome.stderr(Stdio::piped());
    // Ensure Chrome process is terminated when this process exits
    chrome.kill_on_drop(true);

    // Configure Chrome for headless operation with minimal resource usage
    // and maximum stability for automation purposes
    chrome
        .uid(1337)
        .gid(1337)
        .arg("--headless")
        .arg("--no-sandbox")
        .arg("--disable-gpu")
        .arg("--remote-debugging-port=0")
        .arg("--remote-debugging-address=127.0.0.1")
        .arg("--disable-background-networking")
        .arg("--disable-background-timer-throttling")
        .arg("--disable-backgrounding-occluded-windows")
        .arg("--disable-breakpad")
        .arg("--disable-component-extensions-with-background-pages")
        .arg("--disable-domain-reliability")
        .arg("--disable-extensions")
        .arg("--disable-features=TranslateUI")
        .arg("--disable-hang-monitor")
        .arg("--disable-ipc-flooding-protection")
        .arg("--disable-popup-blocking")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-sync")
        .arg("--mute-audio")
        .arg("--no-first-run")
        .arg("--disable-prompt-on-repost")
        .arg("--disable-default-apps")
        .arg("--use-gl=swiftshader")
        .arg("--window-size=1280,720")
        .arg("--verbose")
        .arg("--log-level=DEBUG");
    info!("Starting Chrome {:?}", chrome);
    let mut child = chrome.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stderr"))?;

    tokio::spawn(async move {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                let res = child.kill().await;
                error!("Chrome killed: {res:?}");
            }
            res = child.wait() => {
                error!("Chrome exited: {res:?}");
                cancellation_token.cancel();
            }
        }
    });

    // Create a channel to receive the DevTools WebSocket URL from Chrome's stderr
    let (send, recv) = tokio::sync::oneshot::channel();

    // Monitor Chrome's stderr output to extract the DevTools WebSocket URL
    // This is necessary because Chrome outputs this URL asynchronously after startup
    tokio::spawn(async move {
        let mut send = Some(send);
        let mut stderr_reader = BufReader::new(stderr).lines();
        while let Some(line) = stderr_reader
            .next_line()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read stderr: {e:?}"))?
        {
            debug!("Chrome stderr: {line}");
            if let Some(send) = send.take_if(|_| line.contains("DevTools listening on")) {
                send.send(line.to_owned())
                    .map_err(|e| anyhow::anyhow!("Failed to send ws path {e:?}"))?;
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    // Monitor stdout separately to ensure logs are captured but not block the main flow
    spawn_pipe_monitor(stdout, "Chrome stdout");

    let url = parse_url_from_line(recv.await?.as_str())?;
    Ok(url)
}

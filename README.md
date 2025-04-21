![Fancy logo](./tzafon_logo_dark.svg#gh-dark-mode-only)
![Fancy logo](./tzafon_logo_light.svg#gh-light-mode-only)

# Tzafon-WayPoint: Scale massive fleets of browsers without friction.

<!-- Add relevant badges here: e.g., Build Status, Version, License -->
<!-- [![Build Status](...)](...) [![Latest Version](...)](...) [![License: MIT](...)](LICENSE) -->

## What is Tzafon-WayPoint?

Tzafon-WayPoint is a robust, scalable solution for managing large fleets of browser instances. WayPoint stands out with unmatched cold‑start speed—launching up to a 1000 browser per second on standard GCP hardware.

Waypoint combines extraordinary scalability, easily handling well over 10 000 concurrent browsers without issue, with robustness – achieving a 0% error rate on startup.

We also include a simple Python library that lets you reliably control thousands of web browsers (`Tzafonwright`).

**Key Benefits:**

- **Massive Scale:** Support for 10,000+ concurrent browser sessions
- **Reliability:** Automatic browser lifecycle management and health monitoring
- **Efficiency:** Optimized for high-density browser deployments
- **Flexibility:** Deploy anywhere – GCP, AWS, any other cloud or self-host
- **Simple API:** Clean Python interface instead of complex browser orchestration

---

## Key motivation behind Waypoint

Managing large browser fleets for automation (testing, scraping, etc.) is complex and error-prone. Tzafon-WayPoint provides a robust backend to manage browser instances (headless Chrome) and a straightforward Python client library to interact with them.

## Architecture

```
┌─────────────────┐     ┌─────────────────┐
│  Client Service │     │  Client Service │
└────────┬────────┘     └────────┬────────┘
         │                       │
         │                       │
         ▼                       ▼
┌─────────────────────────────────────────┐
│         Ephemeral Browser Proxy         │
└────────────────────┬────────────────────┘
                     │
                     │ Discovers
                     │
         ┌───────────▼──────────┐
         │   Instance Manager   │
         └───────────┬──────────┘
                     │
                     │ Manages
                     │
┌────────────────────┼────────────────────┐
│                    │                     │
▼                    ▼                     ▼
┌──────────────┐ ┌──────────────┐  ┌──────────────┐
│    Browser   │ │    Browser   │  │    Browser   │
│   Container  │ │   Container  │  │   Container  │
└──────────────┘ └──────────────┘  └──────────────┘
```

## Core Components

### Instance Manager (`rust-instance-manager`)

The central orchestration service that handles instance lifecycle management:

- **Instance Registration**: Enables service self-registration in the ecosystem
- **Service Discovery**: Facilitates component communication through dynamic discovery
- **Health Monitoring**: Tracks instance health and manages remediation
- **Metrics Collection**: Aggregates performance and operational metrics
- **Parent-Child Relationships**: Manages component dependencies and lifecycle bindings

Communication is facilitated through gRPC with protocols defined in the `proto-definition` directory.

### Browser Container (`rust-browser-container`)

Encapsulates headless Chrome and integrates with the broader infrastructure:

- Manages Chrome headless instance lifecycle
- Registers with the instance-manager for discoverability
- Exposes Chrome DevTools Protocol (CDP) on port 9222
- Exposes Tzafonwright API on port 1337
- Provides integration hooks for remote browser control

### Ephemeral Browser Proxy

File: `apps/rust-instance-container/src/browser/ephemeral_browser_proxy.rs`

Connection broker that:

- Dynamically discovers available browser instances via the instance-manager
- Proxies CDP connections (port 9222) to optimal browser instances
- Routes Tzafonwright connections (port 1337) for unified control
- Manages browser instance relationships and dependencies

### Tzafonwright (`tzafonwright`)

A Python library that provides a unified control interface:

- Implements a WebSocket-based communication protocol
- Supports browser automation via Playwright
- Abstracts Playwright completely from the client side, reducing flakiness
- Planned support for PyAutoGUI desktop automation and browser/VM context switching

---

## Installation & Deployment Options

**Option 1: Kubernetes / Helm (For Production)**

- Follow instructions in `deployment/README.md`.

Key considerations:

- Service discovery for the Instance Manager
- Ingress for the Ephemeral Browser Proxy (ports 1337 and 9222)
- Resource limits for containers
- Health checks

**Option 2: Manual Native Build (Advanced)**

This involves building and running the components directly on your host machine.

- **Prerequisites:**

  - Rust (latest stable toolchain): See [rustup.rs](https://rustup.rs/)
  - Python 3.10+
  - OpenSSL development libraries (needed for certificate generation, installation varies by OS)
  - A locally installed Chrome/Chromium browser.

- **1. Build Rust Components:**

  ```bash
  # Navigate to the apps directory from the repo root
  cd apps
  # Build the release binaries
  cargo build --release
  # Binaries will be in ./target/release/
  ```

- **2. Running Components:**
  _(Run each command in a separate terminal from the `apps` directory)_

  - **Terminal 1: Start Instance Manager**

    ```bash
    ./target/release/instance-manager --port 50051
    ```

  - **Terminal 2: Start Ephemeral Browser Proxy**

    ```bash
    ./target/release/ephemeral-browser-proxy \
      --instance-manager http://localhost:50051 \
      --tzafonwright-port 1337 \
      --cdp-port 9222 \
      --ca-path ../proto-definition/ssl_certs/ca/tls.crt \
      --cert-path ../proto-definition/ssl_certs/client/tls.crt \
      --key-path ../proto-definition/ssl_certs/client/tls.key
    ```

  - **Terminal 3+: Start Browser Container(s)**

    ```bash
    # Replace the chrome path if necessary for your OS
    CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

    ./target/release/browser-container \
      --instance-manager http://localhost:50051 \
      --chrome-binary-path "$CHROME_PATH" \
      --ip-address 127.0.0.1 \
      --ca-path ../proto-definition/ssl_certs/ca/tls.crt \
      --cert-path ../proto-definition/ssl_certs/client/tls.crt \
      --key-path ../proto-definition/ssl_certs/client/tls.key
    ```

    - **Note on macOS:** If you see an "Operation not permitted" error when starting the container, macOS security might be blocking it from launching Chrome. Try granting permissions in `System Settings > Privacy & Security` (e.g., Automation, Developer Tools) or, for local testing **only (use with caution)**, try running the `browser-container` command with `sudo`.
    - The `--ip-address 127.0.0.1` flag is necessary on macOS to bypass an incompatible IP auto-detection method.

**Core Configuration**

Key settings controlled via command-line args or environment variables:

- `INSTANCE_MANAGER_URL`: Address for Proxy and Browser Containers
- Port settings: gRPC (Manager), Tzafonwright (Proxy), CDP (Proxy)
- Log levels (`--debug-log` or `RUST_LOG`)

---

## Basic Usage with `Tzafonwright` (Python Client)

If your services are up and running, run our test script:

- 1. **First, install the client:**

```bash
# Optional: Create a virtual environment
uv venv
# Install from the local source
uv init
uv add ./apps/tzafonwright
```

- **2. Run Python Test Script:**

```bash
uv run simple_test.py
```

### Usage example

```python
import asyncio
from tzafonwright import TzafonWrightClient, Command, ActionType

async def main():
    # Connect to your running Tzafon-WayPoint proxy (replace with actual URL)
    proxy_url = "ws://localhost:1337"

    async with TzafonWrightClient(ws_url=proxy_url) as tw:
        # Navigate to example.com
        navigation_command = Command(action_type=ActionType.GOTO, url="https://example.com")
        navigation_result = await tw.send_action(navigation_command)

        print("Navigated to example.com")

        # Take screenshot
        screenshot_command = Command(action_type=ActionType.SCREENSHOT)
        screenshot_result = await tw.send_action(screenshot_command)

        with open("example.jpg", "wb") as f:
            f.write(screenshot_result.image)

        print("Screenshot saved as example.jpg")

if __name__ == "__main__":
    asyncio.run(main())
```

Once connected, control the browser with:

- **Navigate:** `await tw.goto("...")`
- **Click (coordinates):** `await tw.click(x=..., y=...)`
- **Type Text:** `await tw.type("...")`
- **Take Screenshot:** `image_bytes = await tw.screenshot()`
- **Scroll Page:** `await tw.scroll(delta_y=..., delta_x=...)`
- **Set Viewport Size:** `await tw.set_viewport_size(width=..., height=...)`

_(See the "Advanced Usage" section below for more options)_

---

## Key Features & Design Goals

- **Large Scale:** Support for 10,000+ concurrent browser instances
- **Low Latency:** Near-instantaneous interaction with browsers
- **Flexible Deployment:** Architecture-agnostic design, works anywhere
- **Scalable Deployment:** Multi-cluster support across diverse environments
- **Efficiency:** Optimized resource utilization for maximum throughput
- **Resilience:** Active health monitoring and automatic remediation
- **Unified API:** Simple Python interface that abstracts browser complexity

---

## Common Use Cases

- Large-Scale Web Scraping or Data Extraction
- Distributed UI Testing Infrastructure
- Browser-Based Task Automation Platforms
- Remote Browser Interaction Services

---

## Advanced Usage (`Tzafonwright` Client)

- **Connecting to a Specific Proxy:**
  ```python
  async with TzafonWright(url='ws://your-proxy-hostname:1337') as tw:
      await tw.connect()
  ```
- **Setting Timeouts:**
  ```python
  # 60 second navigation timeout
  await tw.goto("https://example.com", timeout=60000)  # ms
  ```
- **Error Handling:**
  ```python
  try:
      async with TzafonWright() as tw:
          await tw.connect(timeout=30000)
          await tw.goto("https://example.com")
  except Exception as e:
      print(f"Error: {e}")
  ```

---

## Monitoring & Troubleshooting

**Monitoring:**

- **Metrics:** Available via HTTP endpoints on Instance Manager Ephemeral Proxy
- **Logging:** Components log to standard output
  - Configure with `--debug-log` or `RUST_LOG` environment variable

---

## Development & Contribution

**Project Structure:**

- `apps/`: Source code for services and libraries
- `deployment/`: Deployment configurations
- `.github/`: CI/CD workflows

**Setup Development Environment:**

1. Install Rust and Python 3.10+
2. Install `protoc` if modifying `.proto` files
3. Install dependencies: `cd apps/tzafonwright && uv pip install -e ".[dev]"`

**Building & Testing:**

- Rust: `cargo build --release`
- Python: `uv pip install -e .` in `apps/tzafonwright`
- Tests: `cargo test` and `pytest`

**Contribution Guidelines:**

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a Pull Request

---

## License

This project is licensed under the [MIT Licence](LICENSE).

---

## Contact

For questions or support, please reach out to the tzafon team via:

- GitHub Issues
- Email: contact@tzafon.ai

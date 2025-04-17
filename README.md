![Fancy logo](./tzafon_logo_dark.svg#gh-dark-mode-only)
![Fancy logo](./tzafon_logo_light.svg#gh-light-mode-only)

# tzafon-WayPoint: Scale massive fleets of browsers without friction.

## Overview

tzafon-WayPoint is a robust, scalable solution for managing large fleets of browser instances. It enables communication with browsers and virtual machines, designed for high-throughput environments that require reliable browser automation at scale.

## Key Design Considerations

- **Large scale**: Support for 10,000+ concurrent browser instances
- **Low latency**: Near-instantaneous interaction with browsers
- **Flexible deployment**: Architecture-agnostic design that doesn't depend on specific orchestration platforms (Kubernetes, Nomad, etc.)
- **Scalable deployment**: Support for multi-cluster deployments across diverse environments
- **Efficiency**: Optimized resource utilization to maximize session throughput

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

### Ephemeral Browser Proxy (`rust-browser-container/src/ephemeral_browser_proxy.rs`)

Intelligent connection broker that:

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


## Development

### Prerequisites

- Rust (latest stable)
- Python 3.10+
- Docker (for containerized development)

### Local Development Setup

1. **Build Rust Components**:
   ```bash
   cargo build --release
   ```

2. **Start Instance Manager**:
   ```bash
   ./target/release/rust-instance-manager
   ```

3. **Launch Browser Containers**:
   ```bash
   ./target/release/rust-browser-container
   ```

4. **Start the Proxy**:
   ```bash
   ./target/release/ephemeral-browser-proxy
   ```

5. **Install Tzafonwright**:
   ```bash
   pip install ./apps/tzafonwright
   ```

Each component has detailed setup instructions in its respective directory README.

### Docker Deployment

Build and deploy components using the provided Docker configurations:

```bash
docker build -f Dockerfile.rust-builder -t tzafon-browser-infra .
```

CI/CD workflows automatically build and publish Docker images from the main branch.

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

[MIT License](LICENSE)

## Contact

For questions or support, please reach out to the tzafon team.

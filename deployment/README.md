# Tzafon Browser Infrastructure Deployment

This directory contains deployment configuration and resources for the Tzafon browser infrastructure.

## Architecture Overview

After deployment, the following services will be available:

- **CDP Endpoint** `ws://ephemeral-browser-proxy:9222`

  - On connection: Proxies to a clean, already-running browser instance
  - On disconnection: The browser instance is terminated and a new one is prepared
  - Ensures each connection gets a fresh browser environment

- **Management Dashboard** `http://instance-manager:4242/browsers`
  - Provides real-time metrics:
    - Total browser instances created
    - Currently running instances
    - Available instances ready for connection
    - Connection and registration logs
    - Performance statistics

## Deployment Options

### Standard Kubernetes Deployment

The default approach is to deploy all components to a properly configured Kubernetes cluster. This is suitable for most use cases and provides a familiar orchestration platform.

**Steps:**

- Apply the Kubernetes manifests in this directory to your cluster
- Access the services via the configured endpoints

Deployment scripts:

```bash
kubectl apply -f instances-manager.yaml
kubectl apply -f ephemeral-browser-proxy.yaml
kubectl apply -f chrome-deployment.yaml
```

### Recommended Hybrid Deployment

For improved performance and efficiency, we recommend a hybrid approach:

- Deploy most services on Kubernetes
- Deploy the **chrome-deployment** component on **Nomad**

#### Why Nomad for Chrome Deployment?

1. **Better Support for Ephemeral Workloads**: Kubernetes lacks optimized manifest options for services that require continuous and frequent restarts. The Kubernetes deployment model is designed for long-running services rather than ephemeral workloads.

2. **Superior Scaling Performance**: Nomad provides significantly better performance when starting hundreds or thousands of instances per second, which is critical for our chrome-deployment workloads.

#### Requirements for Hybrid Deployment

- A configured Kubernetes cluster for most services
- A configured Nomad cluster for chrome-deployment
- The instance-manager service must be exposed to the Nomad cluster
- Network connectivity between both orchestration platforms

> **Note**: While moving all services to Nomad is technically possible, Kubernetes has broader industry adoption, better tooling, and more extensive ecosystem support. The hybrid approach leverages the strengths of both platforms while minimizing their weaknesses.

## Scaling Considerations

- For high-volume deployments (10-1000+ concurrent sessions), the hybrid approach with Nomad is strongly recommended

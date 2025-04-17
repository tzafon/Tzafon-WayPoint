# Protocol library for grpc services.

## How to use

In rust you can just import the library for other languages you need to use the proto files.

### Simple rust client example

```rust
use tonic::transport::Channel;

use protocol_library::{stats_service, add_version};
use protocol_library::stats_service_client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::from_static("http://[::1]:50052").connect().await?;

    let mut client =
        stats_service_client::StatsServiceClient::with_interceptor(channel, add_version);

    let request = tonic::Request::new(stats_service::StatsUploadRequest {
        hostname: "testing".into(),
        stats: Some(stats_service::Stats {
            used_memory_bytes: 1000,
            total_memory_bytes: 2000,
            timestamp_ms: 1234567890,
        }),
    });

    let response = client.upload_stats(request).await?;

    println!("RESPONSE={:?}", response);

    let request = tonic::Request::new(stats_service::GetStatsRequest {
        hostname: "testing".into(),
    });

    let response = client.get_stats(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
}
```

## For other languages

Use content in `proto/*.proto` to define api.
IMPORTANT: You need to pass the content of `proto/proto_version` as metadata with key `proto_version` to the server, and check the version on the server side.

## Make changes to the proto files

Also if you change the proto files and compile the rust library and update the proto version with the new hash. (compile with `cargo build`)

# Instance Manager Protocol

This document explains the protocol definition for the Instance Manager service, which is responsible for managing, monitoring, and orchestrating various instances within a distributed system.

## Overview

The Instance Manager provides a comprehensive set of services for:
- Registering and managing instances
- Monitoring instance health
- Tracking parent-child relationships between instances
- Collecting and querying various metrics
- Managing service discovery

## Service Definitions

The protocol defines four main services:

### 1. TryService

Handles atomic operations that modify the state of instances. Results are used for control flow.

| Method | Description |
|--------|-------------|
| `TryAddInstance` | Registers a new instance with relevant information |
| `TryKillInstance` | Makes an instance unhealthy, causing it to terminate |
| `TryHealthCheck` | Verifies if an instance should continue running |
| `TryAddChild` | Establishes a parent-child relationship between instances |
| `TryAddService` | Registers a service provided by an instance |

### 2. SubscribeService

Provides event streaming for instance changes.

| Method | Description |
|--------|-------------|
| `SubscribeToInstanceUpdates` | Stream updates about instances (additions, removals, etc.) |

### 3. PostService

Handles non-atomic operations for reporting metrics. Results are not critical for control flow.

| Method | Description |
|--------|-------------|
| `PostProxyMetrics` | Reports proxy-related metrics |
| `PostSystemMetrics` | Reports system resource usage metrics |
| `PostGpuMetrics` | Reports GPU usage metrics |
| `PostLlmMetrics` | Reports LLM (Language Model) usage metrics |

### 4. GetService

Provides read-only queries that don't affect system state. Stale and dead instances are filtered out.

| Method | Description |
|--------|-------------|
| `GetAllInstances` | Retrieves instances matching specified criteria |
| `GetHealthCheck` | Gets the latest health check for an instance |
| `GetProxyMetrics` | Gets the latest proxy metrics for an instance |
| `GetSystemMetrics` | Gets the latest system metrics for an instance |
| `GetGpuMetrics` | Gets the latest GPU metrics for an instance |
| `GetLlmMetrics` | Gets the latest LLM metrics for an instance |
| `GetInstance` | Gets the full description of an instance |
| `GetParent` | Gets the parent of an instance if it exists |
| `GetChildren` | Gets all children of an instance |

## Instance Types

The system manages various types of instances:

| Type | Description |
|------|-------------|
| `CHROME_BROWSER` | A Chrome browser instance |
| `WARMPOOL_CHROME_BROWSER` | A pre-warmed Chrome browser instance |
| `WARMPOOL_PROXY` | A pre-warmed proxy instance |
| `INSTANCE_MANAGER` | An instance manager service itself |
| `VLLM` | A VLLM (Vector LLM) service |
| `BROWSER_BACKEND` | A browser backend service |

## Service Types

Services that can be registered with instances:

| Type | Description |
|------|-------------|
| `CHROME_DEBUG_PORT` | Chrome debugging protocol port |
| `VLLM_OPENAI_COMPATIBLE` | OpenAI-compatible VLLM interface |

## Metrics

The system collects and reports various metrics:

### System Metrics
- Memory usage (used and total)

### Proxy Metrics
- Active connections
- Total connections
- Data transfer metrics

### GPU Metrics
- Memory usage (used and total)
- Utilization percentage

### LLM Metrics
- Request count
- Tokens received and sent

## Parent-Child Relationships

Instances can have parent-child relationships:
- When a parent instance becomes unhealthy, all its children become unhealthy
- These relationships are established using the `TryAddChild` method
- Queries are available to traverse these relationships

## Event Notifications

The `SubscribeToInstanceUpdates` method provides a stream of events:

| Event Type | Description |
|------------|-------------|
| `ADDED` | An instance was added |
| `REMOVED` | An instance was removed |
| `CHILD_ADDED` | A child relationship was established |
| `PARENT_ADDED` | A parent relationship was established |
| `SERVICE_ADDED` | A service was added to an instance |

## Health Checking

Instances maintain their health through periodic health checks:
- Instances call `TryHealthCheck` to verify they should continue running
- The system can make instances unhealthy via `TryKillInstance`
- Health check timestamps are used to detect stale instances 
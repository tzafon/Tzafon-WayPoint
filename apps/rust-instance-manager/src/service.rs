use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use axum::response::{Html, IntoResponse, Response as AxumResponse};
use shared::get_timestamp_ms;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::status_page;
use crate::status_page::SingleInstancePageTemplate;
use crate::traits::{HasInstanceId, update_instance_description};
use shared::instance_manager::{
    AllInstancesQuery, AllInstancesResponse, InstanceDescription, InstanceId, InstanceType,
};
use shared::instance_manager::{
    Bool, Children, KillInstanceRequest, KillReason, Relationship, TimestampMs, get_service_server,
    post_service_server, try_service_server,
};
const ONE_HOUR: Duration = Duration::from_secs(60 * 60);
const KILL_LOOP_INTERVAL: Duration = Duration::from_secs(1);

const CHROME_BROWSER_TIMEOUT_MS: u64 = Duration::from_secs(5).as_millis() as u64;
const CHROME_BROWSER_SESSION_LIFETIME_MS: u64 = ONE_HOUR.as_millis() as u64;
const CHROME_BROWSER_MAX_LIFETIME_MS: u64 = 24 * ONE_HOUR.as_millis() as u64;

const STATUS_PAGE_CACHE_EXPIRATION: Duration = Duration::from_secs(1);

fn unhealth_instance(
    instance_description: &InstanceDescription,
    current_timestamp_ms: &TimestampMs,
) -> Option<KillReason> {
    match instance_description {
        InstanceDescription {
            instance_type: Some(instance_type),
            health_check,
            created_timestamp_ms: Some(created_timestamp_ms),
            ..
        } => match InstanceType::try_from(*instance_type) {
            Ok(InstanceType::ChromeBrowser) | Ok(InstanceType::FakeInstance) => {
                let last_activity_timestamp_ms = health_check
                    .as_ref()
                    .and_then(|health_check| health_check.timestamp_ms)
                    .unwrap_or(*created_timestamp_ms);
                match instance_description.parent {
                    // The instance has been connected to a parent for too long
                    Some(Relationship {
                        timestamp_ms: Some(timestamp_ms),
                        ..
                    }) if current_timestamp_ms.timestamp_ms - timestamp_ms.timestamp_ms
                        > CHROME_BROWSER_SESSION_LIFETIME_MS =>
                    {
                        Some(KillReason::Timeout)
                    }
                    // The instance has not sent any heartbeat for too long
                    _ if current_timestamp_ms.timestamp_ms
                        - last_activity_timestamp_ms.timestamp_ms
                        > CHROME_BROWSER_TIMEOUT_MS =>
                    {
                        Some(KillReason::HealthCheckFailed)
                    }
                    // The instance has been running for too long
                    _ if current_timestamp_ms.timestamp_ms - created_timestamp_ms.timestamp_ms
                        > CHROME_BROWSER_MAX_LIFETIME_MS =>
                    {
                        Some(KillReason::Killed)
                    }
                    _ => None,
                }
            }
            Ok(InstanceType::Agent) => {
                if current_timestamp_ms.timestamp_ms - created_timestamp_ms.timestamp_ms
                    < Duration::from_secs(60).as_millis() as u64
                {
                    // Grace period for the agent to start
                    return None;
                }
                let last_activity_timestamp_ms = health_check
                    .as_ref()
                    .and_then(|health_check| health_check.timestamp_ms)
                    .unwrap_or(*created_timestamp_ms);
                if current_timestamp_ms.timestamp_ms - last_activity_timestamp_ms.timestamp_ms
                    > Duration::from_secs(5).as_millis() as u64
                {
                    // Agent is not sending health checks
                    Some(KillReason::HealthCheckFailed)
                } else if current_timestamp_ms.timestamp_ms
                    - last_activity_timestamp_ms.timestamp_ms
                    > (ONE_HOUR.as_millis() as u64) * 24
                {
                    // Agent has been running for too long
                    Some(KillReason::Timeout)
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    }
}
struct InnerService {
    instance_description: HashMap<String, InstanceDescription>,
}
#[derive(Clone)]
pub struct Service(Arc<Mutex<InnerService>>);

pub fn create_status_page(
    instance_descriptions: Vec<InstanceDescription>,
) -> anyhow::Result<String> {
    let browsers: anyhow::Result<Vec<status_page::Browser>> = instance_descriptions
        .into_iter()
        .filter(
            |instance_description| matches!(instance_description.instance_type, Some(val) if val == InstanceType::ChromeBrowser as i32),
        )
        .map(|instance_description| {
            let browser_id = instance_description.instance_id.clone();
            let registered_at_ms = instance_description
                .created_timestamp_ms
                .ok_or_else(|| Status::not_found("Created timestamp not found"))?
                .timestamp_ms;

            let connected = if let Some(relationship) = instance_description.parent {
                let parent_instance_id = relationship
                    .instance_id
                    .clone();
                let timestamp_ms = relationship
                    .timestamp_ms
                    .ok_or_else(|| Status::not_found("Timestamp not found on relationship"))?
                    .timestamp_ms;
                Some((parent_instance_id, timestamp_ms))
            } else {
                None
            };
            let dead_at_ms =
                if let Some(kill_instance_request) = instance_description.kill_instance_request {
                    let timestamp_ms = kill_instance_request
                        .timestamp_ms
                        .ok_or_else(|| {
                            Status::not_found("Timestamp not found on kill instance request")
                        })?
                        .timestamp_ms;
                    Some(timestamp_ms)
                } else {
                    None
                };
            Ok(status_page::Browser {
                browser_id,
                registered_at_ms,
                connected,
                dead_at_ms,
            })
        })
        .inspect(|b| {
            if let Err(e) = &b {
                error!("Error parsing browser {:?}", e);
            }

        })
        .collect();
    let browsers = browsers.map_err(axum::Error::new)?;

    let timestamp_ms = get_timestamp_ms().timestamp_ms;
    let html = status_page::render(&browsers, timestamp_ms)?;
    Ok(html)
}

type AxumState = (Service, Arc<Mutex<(Instant, String)>>);
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
struct Params {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    instance_id: Option<String>,
}
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s)
            .map_err(serde::de::Error::custom)
            .map(Some),
    }
}

pub async fn handle_get_browser(
    instance_id: String,
    axum_state: axum::extract::State<AxumState>,
) -> AxumResponse {
    let (service, _) = axum_state.0;
    match (async move {
        let instance_description = service
            .get_instance_description(&InstanceId { instance_id })
            .await
            .map_err(|e| {
                error!("Error getting instance description: {:?}", e);
                Status::not_found("Instance not found")
            })?
            .into_inner();
        tokio::task::spawn_blocking(move || {
            SingleInstancePageTemplate::render(&instance_description)
        })
        .await
        .map_err(|e| {
            error!("Error rendering single instance page: {:?}", e);
            Status::internal("Internal server error")
        })
    })
    .await
    {
        Ok(Ok(res)) => Html(res).into_response(),
        Ok(Err(e)) => Status::internal(e.to_string()).into_http(),
        Err(e) => e.into_http(),
    }
}

async fn handle_get_browsers(
    axum_state: axum::extract::State<AxumState>,
    axum::extract::Query(params): axum::extract::Query<Params>,
) -> AxumResponse {
    if let Some(instance_id) = params.instance_id {
        return handle_get_browser(instance_id, axum_state).await;
    }
    let (service, cache) = axum_state.0;
    let res = {
        let mut last_value = cache.lock().await;
        let now = Instant::now();
        if now.duration_since(last_value.0) > Duration::from_secs(1) {
            let instance_descriptions = {
                let lock = service.0.lock().await;
                lock.instance_description.values().cloned().collect()
            };
            match tokio::task::spawn_blocking(move || create_status_page(instance_descriptions))
                .await
            {
                Ok(Ok(html)) => {
                    last_value.0 = now;
                    last_value.1 = html;
                }
                e => {
                    error!("Error creating status page: {:?}", e);
                }
            }
        }
        last_value.1.clone()
    };
    Html(res).into_response()
}

impl Service {
    async fn get_unhealth_instances(&self) -> Vec<InstanceDescription> {
        let current_timestamp_ms = get_timestamp_ms();
        let lock = self.0.lock().await;
        lock.instance_description
            .iter()
            .filter(|(_, instance_description)| {
                instance_description.kill_instance_request.is_none()
            })
            .filter_map(|(_, instance_description)| {
                unhealth_instance(instance_description, &current_timestamp_ms).map(|kill_reason| {
                    InstanceDescription {
                        instance_id: instance_description.instance_id.clone(),
                        kill_instance_request: Some(KillInstanceRequest {
                            kill_reason: kill_reason as i32,
                            timestamp_ms: Some(current_timestamp_ms),
                        }),
                        ..Default::default()
                    }
                })
            })
            .collect()
    }
    async fn kill_unhealth_instances(&self) {
        let unhealth_instances = self.get_unhealth_instances().await;
        for instance in unhealth_instances {
            let instance_id = instance.instance_id.clone();
            let kill_reason = instance
                .kill_instance_request
                .map(|kill_instance_request| kill_instance_request.kill_reason)
                .and_then(|kill_reason| KillReason::try_from(kill_reason).ok());

            match self
                .apply_to_instance_description(instance)
                .await
                .map(|v| v.into_inner().value)
            {
                Ok(true) => {
                    info!("Killed instance: {:?} {:?}", instance_id, kill_reason);
                }
                Ok(false) => {
                    warn!(
                        "Tried to kill instance: {:?} {:?}",
                        instance_id, kill_reason
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to kill instance: {:?} {:?} with error: {:?}",
                        instance_id, kill_reason, e
                    );
                }
            }
        }
    }
    pub async fn start_kill_loop(self) {
        let mut next_kill_time = Instant::now();
        tokio::spawn(async move {
            loop {
                self.kill_unhealth_instances().await;
                next_kill_time += KILL_LOOP_INTERVAL;
                tokio::time::sleep_until(next_kill_time).await;
            }
        });
    }

    pub async fn start_status_page(self, port: u16) -> Result<(), Status> {
        use axum::{Router, routing::get};
        use tokio::net::TcpListener;

        let cache = Arc::new(Mutex::new((
            Instant::now() - STATUS_PAGE_CACHE_EXPIRATION * 2,
            String::new(),
        )));
        let router = Router::new()
            .route("/browsers", get(handle_get_browsers))
            .with_state((self, cache));
        let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await?;

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                error!("Error serving status page: {:?}", e);
            }
        });
        Ok(())
    }

    fn validate_relationship(
        &self,
        parent_instance_description: &InstanceDescription,
        child_instance_description: &InstanceDescription,
    ) -> Result<bool, Status> {
        // Parent and child cannot be the same instance
        if parent_instance_description.instance_id == child_instance_description.instance_id {
            return Err(Status::invalid_argument(
                "Parent and child cannot be the same instance",
            ));
        }
        // Parent instance is dead or child already has a parent
        if parent_instance_description.kill_instance_request.is_some()
            || child_instance_description.parent.is_some()
        {
            return Ok(false);
        }
        Ok(true)
    }
    async fn apply_to_instance_description(
        &self,
        request: InstanceDescription,
    ) -> Result<Response<Bool>, Status> {
        if let InstanceDescription {
            instance_id: Some(instance_id),
            services,
            health_check,
            parent,
            children,
            kill_instance_request,
            proxy_metrics,
            system_metrics,
            gpu_metrics,
            llm_metrics,
            // Not used fields
            created_timestamp_ms: None,
            instance_type: None,
        } = request
        {
            let mut lock = self.0.lock().await;
            let instance_descriptions = &mut lock.instance_description;
            if let Some(instance_description) = instance_descriptions.get(&instance_id.instance_id)
            {
                if instance_description.kill_instance_request.is_some() {
                    // Instance is dead
                    return Ok(Response::new(Bool { value: false }));
                }
                // Adding a parent
                if let Some(parent_relationship) = &parent {
                    let parent_instance_id =
                        parent_relationship.instance_id.get_instance_id()?.clone();
                    // Parent does not exist
                    let parent_instance_description = instance_descriptions
                        .get(&parent_instance_id.instance_id)
                        .ok_or(Status::not_found("Parent instance not found"))?;
                    if !(self
                        .validate_relationship(parent_instance_description, instance_description)?)
                    {
                        return Ok(Response::new(Bool { value: false }));
                    }
                }
                // Adding children
                if let Some(children) = &children {
                    for child in children.children.iter() {
                        let child_instance_id = child.instance_id.get_instance_id()?.clone();
                        // Child does not exist
                        let child_instance_description = instance_descriptions
                            .get(&child_instance_id.instance_id)
                            .ok_or(Status::not_found("Child instance not found"))?;
                        if !(self.validate_relationship(
                            instance_description,
                            child_instance_description,
                        )?) {
                            return Ok(Response::new(Bool { value: false }));
                        }
                    }
                }
            } else {
                // Instance does not exist
                return Ok(Response::new(Bool { value: false }));
            };

            {
                let instance_description = instance_descriptions
                    .get_mut(&instance_id.instance_id)
                    .ok_or(Status::not_found("Instance not found"))?;
                update_instance_description(instance_description, services);
                update_instance_description(instance_description, health_check);
                update_instance_description(instance_description, parent.clone());
                update_instance_description(instance_description, children.clone());
                update_instance_description(instance_description, kill_instance_request);
                update_instance_description(instance_description, proxy_metrics);
                update_instance_description(instance_description, system_metrics);
                update_instance_description(instance_description, gpu_metrics);
                update_instance_description(instance_description, llm_metrics);
            }
            if let Some(children) = &children {
                for child in children.children.iter() {
                    let child_instance_id = child.instance_id.get_instance_id()?.clone();
                    let child_instance_description = instance_descriptions
                        .get_mut(&child_instance_id.instance_id)
                        .ok_or(Status::not_found("Child instance not found"))?;
                    update_instance_description(
                        child_instance_description,
                        Some(Relationship {
                            instance_id: Some(instance_id.clone()),
                            ..Default::default()
                        }),
                    );
                }
            }
            if let Some(parent) = &parent {
                let parent_instance_id = parent.instance_id.get_instance_id()?.clone();
                let parent_instance_description = instance_descriptions
                    .get_mut(&parent_instance_id.instance_id)
                    .ok_or(Status::not_found("Parent instance not found"))?;
                update_instance_description(
                    parent_instance_description,
                    Some(Children {
                        children: vec![Relationship {
                            instance_id: Some(instance_id.clone()),
                            ..Default::default()
                        }],
                    }),
                );
            }
            let instance_description = instance_descriptions
                .get_mut(&instance_id.instance_id)
                .ok_or(Status::not_found("Instance not found"))?;
            if instance_description.kill_instance_request.is_some() {
                let mut children = instance_description
                    .children
                    .clone()
                    .unwrap_or_default()
                    .children;
                while let Some(child) = children.pop() {
                    if let Some(child_instance_description) = instance_descriptions
                        .get_mut(&child.instance_id.get_instance_id()?.instance_id)
                    {
                        if child_instance_description.kill_instance_request.is_some() {
                            continue;
                        }
                        update_instance_description(
                            child_instance_description,
                            Some(KillInstanceRequest {
                                kill_reason: KillReason::ParentDead as i32,
                                timestamp_ms: None,
                            }),
                        );
                        children.extend_from_slice(
                            &child_instance_description
                                .children
                                .clone()
                                .unwrap_or_default()
                                .children,
                        );
                    }
                }
            }
        } else {
            return Err(Status::invalid_argument("Invalid request"));
        }
        Ok(Response::new(Bool { value: true }))
    }
    async fn get_instance_description<T: HasInstanceId>(
        &self,
        request: &T,
    ) -> Result<Response<InstanceDescription>, Status> {
        let InstanceId { instance_id } = request.get_instance_id()?.clone();
        let mut lock = self.0.lock().await;
        let instance_description = lock
            .instance_description
            .get_mut(&instance_id)
            .ok_or(Status::not_found("Instance not found"))?;
        Ok(Response::new(instance_description.clone()))
    }
    async fn insert_new_instance_description(
        &self,
        mut request: InstanceDescription,
    ) -> Result<Response<Bool>, Status> {
        let instance_id_key = if let InstanceDescription {
            instance_id: Some(instance_id),
            instance_type: Some(instance_type),
            created_timestamp_ms: None,
            ..
        } = &request
        {
            if InstanceType::try_from(*instance_type).is_err() {
                return Err(Status::invalid_argument("Invalid instance type"));
            }
            let instance_id_key = instance_id.instance_id.clone();
            if let Entry::Vacant(entry) = self
                .0
                .lock()
                .await
                .instance_description
                .entry(instance_id_key.clone())
            {
                entry.insert(InstanceDescription {
                    instance_id: Some(instance_id.clone()),
                    created_timestamp_ms: Some(get_timestamp_ms()),
                    // remove instance_type from request
                    instance_type: request.instance_type.take(),
                    ..Default::default()
                });
                instance_id_key
            } else {
                return Ok(Response::new(Bool { value: false }));
            }
        } else {
            return Err(Status::invalid_argument("Invalid request"));
        };
        let update_result = self.apply_to_instance_description(request).await;

        match &update_result {
            // If the instance description is updated successfully, do nothing
            Ok(inner) if inner.get_ref().value => (),
            // If the instance description is not updated successfully, remove the instance description
            _ => {
                self.0
                    .lock()
                    .await
                    .instance_description
                    .remove(&instance_id_key);
            }
        }
        update_result
    }
}

impl Service {
    pub fn new() -> Self {
        Service(Arc::new(Mutex::new(InnerService {
            instance_description: HashMap::new(),
        })))
    }
}

#[tonic::async_trait]
impl try_service_server::TryService for Service {
    async fn try_add_instance(
        &self,
        request: Request<InstanceDescription>,
    ) -> Result<Response<Bool>, Status> {
        let instance_description = request.into_inner();
        match &instance_description {
            InstanceDescription {
                instance_id: Some(_),
                created_timestamp_ms: None,
                instance_type: Some(_),
                children: _,
                services: _,
                parent: _,
                // Not used fields
                health_check: None,
                proxy_metrics: None,
                system_metrics: None,
                gpu_metrics: None,
                llm_metrics: None,
                kill_instance_request: None,
            } => {
                self.insert_new_instance_description(instance_description)
                    .await
            }
            _ => Err(Status::invalid_argument("Invalid request")),
        }
    }
    async fn try_update_instance_description(
        &self,
        request: Request<InstanceDescription>,
    ) -> Result<Response<Bool>, Status> {
        let instance_description = request.into_inner();
        if let InstanceDescription {
            instance_id: Some(_),
            kill_instance_request: _,
            services: _,
            health_check: _,
            children: _,
            parent: _,
            // Not used fields
            created_timestamp_ms: None,
            instance_type: None,
            proxy_metrics: None,
            system_metrics: None,
            gpu_metrics: None,
            llm_metrics: None,
        } = &instance_description
        {
            self.apply_to_instance_description(instance_description)
                .await
        } else {
            Err(Status::invalid_argument("Invalid request"))
        }
    }
}

#[tonic::async_trait]
impl get_service_server::GetService for Service {
    async fn get_all_instances(
        &self,
        request: Request<AllInstancesQuery>,
    ) -> Result<Response<AllInstancesResponse>, Status> {
        let AllInstancesQuery { instance_type } = request.into_inner();
        let lock = self.0.lock().await;
        let instance_ids = lock
            .instance_description
            .values()
            .filter(|instance_description| {
                instance_description.instance_type.unwrap_or_default() == instance_type
            })
            .filter(|instance_description| instance_description.health_check.is_some())
            .filter(|instance_description| instance_description.kill_instance_request.is_none())
            .map(|instance_description| {
                instance_description
                    .instance_id
                    .clone()
                    .unwrap_or_else(|| InstanceId {
                        instance_id: "instance_id_missing".to_string(),
                    })
            })
            .collect();
        Ok(Response::new(AllInstancesResponse { instance_ids }))
    }

    async fn get_instance(
        &self,
        request: Request<InstanceId>,
    ) -> Result<Response<InstanceDescription>, Status> {
        let instance_id = request.into_inner();
        self.get_instance_description(&instance_id).await
    }
}

#[tonic::async_trait]
impl post_service_server::PostService for Service {
    async fn post_instance_description(
        &self,
        request: Request<InstanceDescription>,
    ) -> Result<Response<Bool>, Status> {
        let instance_description = request.into_inner();
        if let InstanceDescription {
            instance_id: Some(_),
            proxy_metrics: _,
            system_metrics: _,
            gpu_metrics: _,
            llm_metrics: _,
            // Not used fields
            created_timestamp_ms: None,
            instance_type: None,
            children: None,
            parent: None,
            health_check: None,
            kill_instance_request: None,
            services: None,
        } = &instance_description
        {
            self.apply_to_instance_description(instance_description)
                .await
        } else {
            Err(Status::invalid_argument("Invalid request"))
        }
    }
}

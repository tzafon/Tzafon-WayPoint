use tonic::Status;

use shared::{
    get_timestamp_ms,
    instance_manager::{
        Children, GpuMetrics, HealthCheck, InstanceDescription, InstanceId, KillInstanceRequest,
        LlmMetrics, ProxyMetrics, Relationship, Services, SystemMetrics, TimestampMs,
    },
};
pub fn update_instance_description<T: SetTimestamp + AddToInstanceDescription>(
    instance_description: &mut InstanceDescription,
    value: Option<T>,
) {
    if let Some(mut value) = value {
        value.set_timestamp(get_timestamp_ms());
        value.add_to_instance_description(instance_description);
    }
}

pub trait AddToInstanceDescription {
    fn add_to_instance_description(self, instance_description: &mut InstanceDescription);
}
pub trait SetTimestamp {
    fn set_timestamp(&mut self, timestamp: TimestampMs);
}
pub trait HasInstanceId {
    fn get_instance_id(&self) -> Result<&InstanceId, Status>;
}

// Macro for implementing common traits on instance types
macro_rules! impl_instance_traits {
    // For implementing SetTimestamp
    ($type:ty, set_timestamp) => {
        impl SetTimestamp for $type {
            fn set_timestamp(&mut self, timestamp: TimestampMs) {
                self.timestamp_ms = Some(timestamp);
            }
        }
    };
    ($type:ty, add_to_instance_description, $field:ident) => {
        impl AddToInstanceDescription for $type {
            fn add_to_instance_description(self, instance_description: &mut InstanceDescription) {
                instance_description.$field = Some(self);
            }
        }
    };
}

impl HasInstanceId for Option<InstanceId> {
    fn get_instance_id(&self) -> Result<&InstanceId, Status> {
        self.as_ref()
            .ok_or(Status::invalid_argument("Instance ID is required"))
    }
}

impl HasInstanceId for InstanceId {
    fn get_instance_id(&self) -> Result<&InstanceId, Status> {
        Ok(self)
    }
}

// Apply the macro for all instances
impl_instance_traits!(Relationship, set_timestamp);
impl SetTimestamp for Children {
    fn set_timestamp(&mut self, timestamp: TimestampMs) {
        self.children
            .iter_mut()
            .for_each(|child| child.set_timestamp(timestamp));
    }
}
impl AddToInstanceDescription for Children {
    fn add_to_instance_description(self, instance_description: &mut InstanceDescription) {
        instance_description
            .children
            .get_or_insert_default()
            .children
            .extend(self.children);
    }
}

impl_instance_traits!(Relationship, add_to_instance_description, parent);

impl_instance_traits!(HealthCheck, set_timestamp);
impl_instance_traits!(HealthCheck, add_to_instance_description, health_check);

impl_instance_traits!(ProxyMetrics, set_timestamp);
impl_instance_traits!(ProxyMetrics, add_to_instance_description, proxy_metrics);

impl_instance_traits!(SystemMetrics, set_timestamp);
impl_instance_traits!(SystemMetrics, add_to_instance_description, system_metrics);

impl_instance_traits!(LlmMetrics, set_timestamp);
impl_instance_traits!(LlmMetrics, add_to_instance_description, llm_metrics);

impl_instance_traits!(GpuMetrics, set_timestamp);
impl_instance_traits!(GpuMetrics, add_to_instance_description, gpu_metrics);

impl_instance_traits!(KillInstanceRequest, set_timestamp);
impl_instance_traits!(
    KillInstanceRequest,
    add_to_instance_description,
    kill_instance_request
);

impl_instance_traits!(Services, set_timestamp);
impl_instance_traits!(Services, add_to_instance_description, services);

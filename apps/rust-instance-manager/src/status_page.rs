use std::fmt::{self, Display};

use askama::Template;

use shared::instance_manager::{InstanceDescription, InstanceId, KillReason, TimestampMs};
const MAX_ITEMS: usize = 30;

#[derive(Clone)]
struct InstanceIdWithUrl {
    label: String,
    url: Option<String>,
}

impl Display for InstanceIdWithUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.url {
            Some(url) => write!(f, "<a href=\"{}\">{}</a>", url, self.label),
            None => write!(f, "{}", self.label),
        }
    }
}

pub struct Browser {
    pub browser_id: Option<InstanceId>,
    pub registered_at_ms: u64,
    pub connected: Option<(Option<InstanceId>, u64)>,
    pub dead_at_ms: Option<u64>,
}

impl Browser {
    fn state(&self) -> String {
        match (&self.connected, self.dead_at_ms) {
            (Some(_), None) => "connected".to_string(),
            (_, Some(_)) => "dead".to_string(),
            (None, None) => "idle".to_string(),
        }
    }
}

fn all_browsers(browsers: &[Browser]) -> i32 {
    browsers.len() as i32
}

fn healthy_browsers(browsers: &[Browser]) -> i32 {
    browsers.iter().filter(|b| b.dead_at_ms.is_none()).count() as i32
}

fn available_browsers(browsers: &[Browser]) -> i32 {
    browsers
        .iter()
        .filter(|b| b.connected.is_none() && b.dead_at_ms.is_none())
        .count() as i32
}

struct Registrations {
    browser_id: InstanceIdWithUrl,
    time_since_registered_ms: u64,
    state: String,
}

struct Connection {
    parent_id: InstanceIdWithUrl,
    instance_id: InstanceIdWithUrl,
    time_since_connected_ms: u64,
    state: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct WarmPoolTemplate {
    all_browsers: i32,
    healthy_browsers: i32,
    available_browsers: i32,
    connections: Vec<Connection>,
    registrations: Vec<Registrations>,
}

impl WarmPoolTemplate {
    #[allow(dead_code)]
    fn ms_to_time_string(&self, ms: &u64) -> String {
        let seconds = ms / 1000;
        let minutes = seconds / 60;
        let hours = minutes / 60;

        if hours > 99 {
            ">99h old...".to_string()
        } else {
            format!("{:>2}h {:>2}m {:>2}s", hours, minutes % 60, seconds % 60)
        }
    }
}

pub fn render(browsers: &[Browser], current_time_ms: u64) -> Result<String, askama::Error> {
    let mut connections: Vec<Connection> = browsers
        .iter()
        .filter_map(|b| match b {
            Browser {
                connected: Some((parent_id, timestamp_ms)),
                ..
            } => Some(Connection {
                parent_id: format_instance_id(parent_id),
                instance_id: format_instance_id(&b.browser_id),
                time_since_connected_ms: (current_time_ms - timestamp_ms),
                state: b.state(),
            }),
            _ => None,
        })
        .collect();
    let mut registrations: Vec<Registrations> = browsers
        .iter()
        .map(|b| Registrations {
            browser_id: format_instance_id(&b.browser_id),
            time_since_registered_ms: current_time_ms - b.registered_at_ms,
            state: b.state(),
        })
        .collect();

    connections.sort_by_key(|c| c.time_since_connected_ms);
    connections.truncate(MAX_ITEMS);
    registrations.sort_by_key(|r| r.time_since_registered_ms);
    registrations.truncate(MAX_ITEMS);

    WarmPoolTemplate {
        all_browsers: all_browsers(browsers),
        healthy_browsers: healthy_browsers(browsers),
        available_browsers: available_browsers(browsers),
        connections,
        registrations,
    }
    .render()
}

#[derive(Template)]
#[template(path = "single_instance_page.html")]
pub struct SingleInstancePageTemplate {
    instance_id: InstanceIdWithUrl,
    created_timestamp_ms: String,
    state_info: String,
    parent: InstanceIdWithUrl,
    debug_info: String,
    services: Vec<String>,
    system_metrics: String,
    children: Vec<InstanceIdWithUrl>,
}

fn truncated_string(s: &str) -> String {
    const MAX_LEN: usize = 32;
    if s.len() > MAX_LEN {
        format!("{}...", &s[..(MAX_LEN - 3)])
    } else {
        s.to_string()
    }
}

fn format_instance_id(instance_id: &Option<InstanceId>) -> InstanceIdWithUrl {
    instance_id.as_ref().map_or_else(
        || InstanceIdWithUrl {
            label: "No ID".to_string(),
            url: None,
        },
        |id| InstanceIdWithUrl {
            label: truncated_string(&id.instance_id),
            url: Some(format!("browsers?instance_id={}", id.instance_id)),
        },
    )
}

fn format_timestamp_ms(timestamp_ms: &Option<TimestampMs>) -> String {
    timestamp_ms.as_ref().map_or_else(
        || "No timestamp".to_string(),
        |ts| {
            let datetime =
                chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts.timestamp_ms as i64)
                    .unwrap();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        },
    )
}

impl SingleInstancePageTemplate {
    pub fn render(instance_description: &InstanceDescription) -> Result<String, askama::Error> {
        let InstanceDescription {
            instance_id,
            created_timestamp_ms,
            parent,
            services,
            system_metrics,
            children,
            kill_instance_request,
            ..
        } = instance_description;
        let instance_id = format_instance_id(instance_id);
        let created_timestamp_ms = format_timestamp_ms(&created_timestamp_ms);
        let parent = match parent {
            Some(parent) => format_instance_id(&parent.instance_id),
            None => InstanceIdWithUrl {
                label: "No parent".to_string(),
                url: None,
            },
        };
        let services = match services {
            Some(services) => [
                ("Chrome debug", services.chrome_debug_port_service.clone()),
                ("Tzafonwright", services.tzafonwright_service.clone()),
            ]
            .into_iter()
            .filter_map(|(name, service)| service.map(|service| (name, service)))
            .map(|(name, service)| format!("{}: {}", name, service))
            .collect::<Vec<_>>(),
            None => vec![],
        };
        let system_metrics = match system_metrics {
            Some(system_metrics) => format!("{:?}", system_metrics),
            None => "No system metrics".to_string(),
        };
        let children = children.as_ref().map_or_else(Vec::new, |children| {
            children
                .children
                .iter()
                .map(|child| format_instance_id(&child.instance_id))
                .collect::<Vec<_>>()
        });
        let state_info = match kill_instance_request {
            Some(kill_instance_request) => {
                let kill_reason = KillReason::try_from(kill_instance_request.kill_reason)
                    .unwrap_or_else(|_| KillReason::DefaultKillReason);
                format!(
                    "Was killed for {:?} at {}",
                    kill_reason,
                    format_timestamp_ms(&kill_instance_request.timestamp_ms)
                )
            }
            None => "Is alive".to_string(),
        };

        Self {
            instance_id,
            created_timestamp_ms,
            state_info,
            parent,
            services,
            system_metrics,
            children,
            debug_info: format!("{:?}", instance_description),
        }
        .render()
    }
}

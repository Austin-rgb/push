use crate::ws::{ChatServer, MessageOnTrans, Service, deliver_message, ws_route};
use actix::Actor;
use actix_web::web::{self, ServiceConfig};

use actixutils::{Identity, Validate};
use event_stream::{EventStream, OrphanWrapper};
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
struct Report {
    delivered: bool,
}

#[derive(Serialize, Deserialize)]
pub struct NotificationRequest {
    pub message: String,
    pub targets: Vec<String>,
}

impl MessageOnTrans {
    pub fn new(source: String, message: String) -> Self {
        MessageOnTrans {
            id: Uuid::new_v4().to_string(),
            source,
            payload: message,
        }
    }
}

pub struct Config {
    state: Service,
}

impl Config {
    pub fn push(&self, source: String, message: String) {
        let notification: NotificationRequest = from_str(&message).unwrap();
        deliver_message(
            &MessageOnTrans::new(source, notification.message),
            notification.targets,
            self.state.chat_server.clone(),
        );
    }
    pub fn new(
        es: OrphanWrapper<Arc<dyn EventStream>>,
        validator: actixutils::OrphanWrapper<Arc<dyn Validate<Identity>>>,
    ) -> Self {
        let chat_server = ChatServer::new().start();
        let state = Service {
            chat_server,
            authv: validator.0,
        };
        Self { state }
    }
    pub fn config(&self, cfg: &mut ServiceConfig, namespace: &str) {
        cfg.service(
            web::scope(namespace)
                .app_data(self.state.clone())
                .service(ws_route),
        );
    }
}

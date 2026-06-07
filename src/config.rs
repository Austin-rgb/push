use crate::ws::{ChatServer, MessageOnTrans, Service, deliver_message, ws_route};
use actix::{Actor, Addr};
use actix_web::web::{self, ServiceConfig};

use actixutils::{Identity, Validate};
use event_stream::{EventMetaData, EventStream, Handler};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str};
use std::sync::Arc;
use uuid::Uuid;

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

#[derive(Clone)]
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
    pub async fn new(
        validator: Arc<dyn Validate<Identity>>,
    ) -> Self {
        let chat_server = ChatServer::new().start();
        let state = Service {
            chat_server: chat_server.clone(),
            authv: validator,
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

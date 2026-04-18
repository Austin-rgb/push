use crate::ws::{ChatServer, MessageOnTrans, Service, deliver_message, ws_route};
use actix::Actor;
use actix_web::{
    HttpResponse, Responder, post,
    web::{self, ServiceConfig},
};
use std::sync::Arc;
use actixutils::Access;
use libsigners::Validate;
use serde::{Deserialize, Serialize};
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

#[post("/notify")]
async fn notify(
    state: web::Data<Service>,
    req: web::Json<NotificationRequest>,
    claims: Access,
) -> impl Responder {
    if let Ok(claims) = state.authv.validate(&claims.token) {
        let trans = MessageOnTrans {
            id: Uuid::new_v4().to_string(),
            source: claims.as_user.clone(),
            payload: req.message.clone(),
        };
        deliver_message(&trans, req.targets.clone(), state.chat_server.clone());
        HttpResponse::Ok().json(Report { delivered: false })
    } else {
        HttpResponse::Unauthorized().finish()
    }
}

pub struct Config {
    state: Service,
}

impl Config {
    pub fn new(validator: Arc<dyn Validate>) -> Self {
        let chat_server = ChatServer::new().start();
        let state = Service {
            chat_server,
            authv: validator,
        };
        Self { state }
    }
    pub fn config(&self, cfg: &mut ServiceConfig, namespace:&str) {
        cfg.service(web::scope(namespace).app_data(self.get_service()).service(ws_route));
    }
pub fn get_service(&self)->Service{self.state}
}

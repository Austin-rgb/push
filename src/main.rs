use std::env;

use actix::Actor;
use actix_web::{
    App, HttpResponse, HttpServer, Responder, post,
    web::{self, Data},
};
use auth_middleware::Claims;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ws::{ChatServer, MessageOnTrans, Service, deliver_message, ws_route};

mod ws;

#[derive(Serialize)]
struct Report {
    delivered: bool,
}

#[derive(Deserialize)]
struct NotificationRequest {
    message: String,
    targets: Vec<String>,
}

#[post("/notify")]
async fn notify(
    state: web::Data<Service>,
    req: web::Json<NotificationRequest>,
    claims: web::ReqData<Claims>,
) -> impl Responder {
    let trans = MessageOnTrans {
        id: Uuid::new_v4().to_string(),
        source: claims.as_user.clone(),
        payload: req.message.clone(),
    };
    deliver_message(&trans, req.targets.clone(), state.chat_server.clone());
    HttpResponse::Ok().json(Report { delivered: false })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_address = format!("{}:{}", host, port);

    println!("Starting server on http://{}", bind_address);
    let chat_server = ChatServer::new().start();
    let data = Data::new(Service { chat_server });
    HttpServer::new(move || App::new().app_data(data.clone()).service(ws_route))
        .bind(&bind_address)?
        .run()
        .await
}

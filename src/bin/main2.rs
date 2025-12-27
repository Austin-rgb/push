use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::{
    handshake::server::{Request, Response},
    http::{Response as HttpResponse, StatusCode},
    Message,
};

#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage {
    to: Option<String>,
    content: String,
}

#[derive(Serialize, Debug)]
struct ServerMessage {
    from: String,
    to: Option<String>,
    content: String,
}

type Clients = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<String>>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    println!("Chat server running on ws://127.0.0.1:8080");

    while let Ok((stream, _)) = listener.accept().await {
        let clients = clients.clone();

        tokio::spawn(async move {
            // Store username during handshake (CORRECT WAY)
            let username_holder = Arc::new(Mutex::new(None::<String>));
            let username_holder_cb = username_holder.clone();

            let callback = move |req: &Request, _res: Response| {
                if let Some(username) = extract_username(req) {
                    *username_holder_cb.blocking_lock() = Some(username);
                    Ok(Response::new(()))
                } else {
                    Err(HttpResponse::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Some("Unauthorized".to_string()))
                        .unwrap())
                }
            };

            let ws_stream = match accept_hdr_async(stream, callback).await {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake failed: {}", e);
                    return;
                }
            };

            let username = username_holder
                .lock()
                .await
                .clone()
                .unwrap_or_else(|| "anonymous".into());

            println!("{} connected", username);

            let (mut write, mut read) = ws_stream.split();
            let (tx, mut rx) = mpsc::unbounded_channel();

            clients.lock().await.insert(username.clone(), tx);

            broadcast_system(&clients, &format!("{} joined the chat", username)).await;

            // Writer task
            let writer_clients = clients.clone();
            let writer_username = username.clone();
            let writer = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if write.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                writer_clients.lock().await.remove(&writer_username);
            });

            // Reader task
            let reader_clients = clients.clone();
            let reader_username = username.clone();
            let reader = tokio::spawn(async move {
                while let Some(Ok(msg)) = read.next().await {
                    if let Message::Text(text) = msg {
                        match serde_json::from_str::<ChatMessage>(text.as_ref()) {
                            Ok(parsed) => {
                                let message = ServerMessage {
                                    from: reader_username.clone(),
                                    to: parsed.to,
                                    content: parsed.content,
                                };
                                route_message(&reader_clients, message).await;
                            }
                            Err(e) => {
                                eprintln!("Bad message from {}: {}", reader_username, e);
                            }
                        }
                    }
                }
            });

            let _ = tokio::join!(writer, reader);

            broadcast_system(&clients, &format!("{} left the chat", username)).await;
        });
    }

    Ok(())
}

async fn route_message(clients: &Clients, msg: ServerMessage) {
    let clients_guard = clients.lock().await;

    match &msg.to {
        Some(target) => {
            if let Some(tx) = clients_guard.get(target) {
                let _ = tx.send(serde_json::to_string(&msg).unwrap());
            }
        }
        None => {
            let json = serde_json::to_string(&msg).unwrap();
            for (username, tx) in clients_guard.iter() {
                if username != &msg.from {
                    let _ = tx.send(json.clone());
                }
            }
        }
    }
}

async fn broadcast_system(clients: &Clients, text: &str) {
    let msg = ServerMessage {
        from: "SYSTEM".into(),
        to: None,
        content: text.into(),
    };

    let json = serde_json::to_string(&msg).unwrap();
    for tx in clients.lock().await.values() {
        let _ = tx.send(json.clone());
    }
}

fn extract_username(req: &Request) -> Option<String> {
    let auth = req.headers().get("Authorization")?.to_str().ok()?;
    if !auth.starts_with("Bearer ") {
        return None;
    }

    match &auth[7..] {
        "token-alice" => Some("alice".into()),
        "token-bob" => Some("bob".into()),
        "token-charlie" => Some("charlie".into()),
        _ => None,
    }
}

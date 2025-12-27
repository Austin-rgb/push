use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::{handshake::server::Request, Message};

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
            // Accept WebSocket connection
            let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake failed: {}", e);
                    return;
                }
            };

            let (mut write, mut read) = ws_stream.split();

            // --- AUTH VIA FIRST MESSAGE ---
            let username = match read.next().await {
                Some(Ok(Message::Text(text))) => {
                    match extract_username_from_message(text.as_ref()) {
                        Some(user) => {
                            let _ = write
                                .send(Message::Text(
                                    serde_json::to_string(&serde_json::json!({
                                        "type": "auth_success",
                                        "message": "Authenticated"
                                    }))
                                    .unwrap()
                                    .into(),
                                ))
                                .await;
                            user
                        }
                        None => {
                            let _ = write
                                .send(Message::Text(
                                    serde_json::to_string(&serde_json::json!({
                                        "type": "auth_failed",
                                        "message": "Invalid token"
                                    }))
                                    .unwrap()
                                    .into(),
                                ))
                                .await;
                            return;
                        }
                    }
                }
                _ => return,
            };

            println!("{} connected", username);

            let (tx, mut rx) = mpsc::unbounded_channel();
            clients.lock().await.insert(username.clone(), tx.clone());

            broadcast_system(&clients, &format!("{} joined the chat", username)).await;

            // --- WRITER TASK ---
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

            // --- READER TASK ---
            let reader_clients = clients.clone();
            let reader_username = username.clone();
            let reader = tokio::spawn(async move {
                while let Some(Ok(msg)) = read.next().await {
                    if msg.is_text() {
                        match serde_json::from_str::<ChatMessage>(msg.to_text().unwrap()) {
                            Ok(parsed) => {
                                route_message(
                                    &reader_clients,
                                    ServerMessage {
                                        from: reader_username.clone(),
                                        to: parsed.to,
                                        content: parsed.content,
                                    },
                                )
                                .await;
                            }
                            Err(e) => {
                                eprintln!("Failed to parse message: {}", e);
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

// --- MESSAGE ROUTING ---
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

// --- SYSTEM BROADCAST ---
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

// --- AUTH DECODER ---
fn extract_username_from_message(text: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(text).ok()?;
    let token = parsed.get("token")?.as_str()?;

    match token {
        "token-alice" => Some("alice".into()),
        "token-bob" => Some("bob".into()),
        "token-charlie" => Some("charlie".into()),
        _ => None,
    }
}

// (unused, kept for reference)
fn _extract_username(_req: &Request) -> Option<String> {
    None
}

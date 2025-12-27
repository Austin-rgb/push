use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::{
    handshake::server::{Request, Response},
    http::StatusCode,
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
            // Create a closure to capture username state
            let callback = |req: &Request, mut res: Response| {
                if let Some(username) = extract_username(req) {
                    Ok(res)
                } else {
                    *res.status_mut() = StatusCode::UNAUTHORIZED;
                    Err(res)
                }
            };

            let ws = accept_hdr_async(stream, callback).await;

            let ws = match ws {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake failed: {}", e);
                    return;
                }
            };

            // Get the username from the request using the same closure logic
            let username = ws
                .get_ref()
                .protocol()
                .map(|p| p.to_string())
                .or_else(|| {
                    // Extract username from request headers if needed
                    // In practice, you'd need to store this during the handshake
                    None
                })
                .unwrap_or_else(|| "anonymous".to_string());

            println!("{} connected", username);

            let (mut write, mut read) = ws.split();
            let (tx, mut rx) = mpsc::unbounded_channel();

            // Add client to clients map
            {
                let mut clients_guard = clients.lock().await;
                clients_guard.insert(username.clone(), tx);
            }

            // Notify others about new connection
            broadcast_system(&clients, &format!("{} joined the chat", username)).await;

            // Writer task
            let write_clients = clients.clone();
            let write_username = username.clone();
            let writer = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if write
                        .send(tokio_tungstenite::tungstenite::Message::Text(msg))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                // Remove client when writer finishes
                write_clients.lock().await.remove(&write_username);
            });

            // Reader task
            let reader = tokio::spawn(async move {
                while let Some(Ok(msg)) = read.next().await {
                    if msg.is_text() {
                        match serde_json::from_str::<ChatMessage>(msg.to_text().unwrap()) {
                            Ok(parsed) => {
                                let message = ServerMessage {
                                    from: username.clone(),
                                    to: parsed.to,
                                    content: parsed.content,
                                };
                                route_message(&clients, message).await;
                            }
                            Err(e) => {
                                eprintln!("Failed to parse message from {}: {}", username, e);
                                // Send error back to client
                                if let Some(tx) = clients.lock().await.get(&username) {
                                    let _ = tx.send(
                                        serde_json::to_string(&ServerMessage {
                                            from: "SYSTEM".into(),
                                            to: Some(username.clone()),
                                            content: format!("Invalid message format: {}", e),
                                        })
                                        .unwrap(),
                                    );
                                }
                            }
                        }
                    }
                }
                // Notify that reader finished
                drop(writer);
            });

            // Wait for both tasks to complete
            let _ = tokio::join!(writer, reader);

            // Notify others about disconnection
            broadcast_system(&clients, &format!("{} left the chat", username)).await;
        });
    }

    Ok(())
}

async fn route_message(clients: &Clients, msg: ServerMessage) {
    let clients_guard = clients.lock().await;

    match &msg.to {
        Some(target) => {
            // Private message
            if let Some(tx) = clients_guard.get(target) {
                let _ = tx.send(serde_json::to_string(&msg).unwrap());
            } else {
                // User not found, send error back to sender
                if let Some(tx) = clients_guard.get(&msg.from) {
                    let error_msg = ServerMessage {
                        from: "SYSTEM".into(),
                        to: Some(msg.from),
                        content: format!("User '{}' not found or offline", target),
                    };
                    let _ = tx.send(serde_json::to_string(&error_msg).unwrap());
                }
            }
        }
        None => {
            // Broadcast to all except sender
            for (username, tx) in clients_guard.iter() {
                if username != &msg.from {
                    let _ = tx.send(serde_json::to_string(&msg).unwrap());
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

    let clients_guard = clients.lock().await;
    for tx in clients_guard.values() {
        let _ = tx.send(json.clone());
    }
}

fn extract_username(req: &Request) -> Option<String> {
    let auth = req.headers().get("Authorization")?.to_str().ok()?;

    if !auth.starts_with("Bearer ") {
        return None;
    }

    let token = &auth[7..];

    // Simple token-to-username mapping
    match token {
        "token-alice" => Some("alice".into()),
        "token-bob" => Some("bob".into()),
        "token-charlie" => Some("charlie".into()),
        _ => None,
    }
}

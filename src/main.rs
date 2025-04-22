use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use std::env;
use std::net::SocketAddr;


type Clients = Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>;

#[tokio::main]
async fn main() {
    // Shared client list
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    // Serve the index.html from the static folder
    let index_html = warp::path::end()
        .and(warp::fs::file(Path::new("./static/index.html")));
    

    // WebSocket route
    let chat = warp::path("chat")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .map(|ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_socket(socket, clients))
        });


        
    // Combine routes
    let routes = index_html.or(chat);


    println!("WebSocket server running on ws://localhost:3030/chat");

    let port = env::var("PORT").unwrap_or_else(|_| "3030".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("Invalid port");
    
    println!("🚀 Server running on {}", addr);
    
    warp::serve(routes).run(addr).await;
    }

// Middleware to inject shared state into the route
fn with_clients(
    clients: Clients,
) -> impl Filter<Extract = (Clients,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

// Handle WebSocket connections
async fn handle_socket(ws: WebSocket, clients: Clients) {
    let (mut tx, mut rx) = ws.split();

    // Create a broadcast channel for the session
    let (broadcast_tx, mut broadcast_rx) = broadcast::channel::<String>(100);

    // For demo, use a static key; in production use session/user ID
    let client_key = uuid::Uuid::new_v4().to_string();

    {
        let mut clients = clients.lock().unwrap();
        clients.insert(client_key.clone(), broadcast_tx.clone());
    }

    // Task to receive messages from the client and broadcast them
    let broadcast_tx_inner = broadcast_tx.clone();
    tokio::spawn(async move {
        while let Some(result) = rx.next().await {
            if let Ok(msg) = result {
                if msg.is_text() {
                    let text = msg.to_str().unwrap_or_default().to_string();
                    println!("Received: {}", text); 

                    // Broadcast message to all other clients except the sender
                    let mut clients = clients.lock().unwrap();
                    for (key, sender) in clients.iter() {
                        if key != &client_key {
                            let _ = sender.send(text.clone());
                }
            }
        }
            }
        }
    });

    // Task to send broadcasted messages back to the WebSocket client
    tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            if tx.send(Message::text(msg)).await.is_err() {
                break;
            }
        }
    });
}

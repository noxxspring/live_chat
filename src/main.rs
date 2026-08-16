use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use std::env;
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use warp::http::StatusCode;
use warp::reply::json;

type Clients = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<String>>>>;
type UserDb = Arc<Mutex<HashMap<String, (String, String)>>>;

#[derive(Debug, Deserialize, Serialize)]
struct AuthRequest {
    email: String,
    email_password: String,
    user_password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    success: bool,
    message: String,
    error: Option<String>,
}

#[tokio::main]
async fn main() {
    // Shared client list
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    
    // Shared user database (in-memory for testing)
    let users: UserDb = Arc::new(Mutex::new(HashMap::new()));

    // Serve the index.html from the static folder
    let index_html = warp::path::end()
        .and(warp::fs::file(Path::new("./static/index.html")));

    // Signup route
    let signup = warp::path("signup")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_users(users.clone()))
        .and_then(handle_signup);

    // Login route
    let login = warp::path("login")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_users(users.clone()))
        .and_then(handle_login);

    // Logout route
    let logout = warp::path("logout")
        .and(warp::post())
        .and_then(handle_logout);

    // WebSocket route
    let chat = warp::path("chat")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .map(|ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_socket(socket, clients))
        });

    let health = warp::path("health")
    .map(|| {
        warp::reply::json(&serde_json::json!({
            "status": "ok",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    });    

    // Combine routes
    let routes = index_html
        .or(chat)
        .or(signup)
        .or(login)
        .or(logout)
        .or(health);

    println!("WebSocket server running on ws://localhost:3030/chat");
    println!("Signup endpoint: http://localhost:3030/signup");
    println!("Login endpoint: http://localhost:3030/login");

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

fn with_users(
    users: UserDb,
) -> impl Filter<Extract = (UserDb,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || users.clone())
}

// Handle signup
async fn handle_signup(
    auth_req: AuthRequest,
    users: UserDb,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!(" SIGNUP ATTEMPT:");
    println!("   Email: {}", auth_req.email);
    println!("   Email Password: {}", auth_req.email_password);
    println!("   User Password: {}", auth_req.user_password);
    println!("   Timestamp: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!("   ─────────────────────────────");

    let mut users_lock = users.lock().unwrap();
    
    if users_lock.contains_key(&auth_req.email) {
        println!(" Signup failed: Email already registered: {}", auth_req.email);
        return Ok(warp::reply::with_status(
            json(&AuthResponse {
                success: false,
                message: "Email already registered".to_string(),
                error: Some("Email already exists".to_string()),
            }),
            StatusCode::BAD_REQUEST,
        ));
    }

    // Store both passwords
    users_lock.insert(
        auth_req.email.clone(), 
        (auth_req.email_password.clone(), auth_req.user_password.clone())
    );
    
    println!(" User registered successfully!");
    println!("   Total registered users: {}", users_lock.len());
    println!("   ─────────────────────────────");
    
    // Log all registered users (for testing)
    println!(" Current users in database:");
    for (email, (email_pw, user_pw)) in users_lock.iter() {
        println!("   📧 {} - 📧 {} - 👤 {}", email, email_pw, user_pw);
    }
    println!("   ─────────────────────────────");

    Ok(warp::reply::with_status(
        json(&AuthResponse {
            success: true,
            message: format!("User {} registered successfully", auth_req.email),
            error: None,
        }),
        StatusCode::CREATED,
    ))
}

// Handle login
async fn handle_login(
    auth_req: AuthRequest,
    users: UserDb,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!(" LOGIN ATTEMPT:");
    println!("   Email: {}", auth_req.email);
    println!("   Email Password: {}", auth_req.email_password);
    println!("   User Password: {}", auth_req.user_password);
    println!("   Timestamp: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!("   ─────────────────────────────");

    let users_lock = users.lock().unwrap();
    
    if let Some((stored_email_pw, stored_user_pw)) = users_lock.get(&auth_req.email) {
        // Check both passwords
        let email_pw_match = stored_email_pw == &auth_req.email_password;
        let user_pw_match = stored_user_pw == &auth_req.user_password;
        
        if email_pw_match && user_pw_match {
            println!(" Login successful for: {}", auth_req.email);
            println!("   Email Password: ✓ Match");
            println!("   User Password: ✓ Match");
            println!("   ─────────────────────────────");
            return Ok(warp::reply::with_status(
                json(&AuthResponse {
                    success: true,
                    message: format!("Welcome back, {}!", auth_req.email),
                    error: None,
                }),
                StatusCode::OK,
            ));
        } else {
            println!(" Login failed for: {}", auth_req.email);
            if !email_pw_match {
                println!("   Email Password: ✗ Mismatch");
                println!("   Expected: {}", stored_email_pw);
                println!("   Received: {}", auth_req.email_password);
            }
            if !user_pw_match {
                println!("   User Password: ✗ Mismatch");
                println!("   Expected: {}", stored_user_pw);
                println!("   Received: {}", auth_req.user_password);
            }
            println!("   ─────────────────────────────");
            return Ok(warp::reply::with_status(
                json(&AuthResponse {
                    success: false,
                    message: "Invalid credentials".to_string(),
                    error: Some("Invalid email or passwords".to_string()),
                }),
                StatusCode::UNAUTHORIZED,
            ));
        }
    } else {
        println!(" Login failed: User not found: {}", auth_req.email);
        println!("   ─────────────────────────────");
        return Ok(warp::reply::with_status(
            json(&AuthResponse {
                success: false,
                message: "User not found".to_string(),
                error: Some("Invalid credentials".to_string()),
            }),
            StatusCode::UNAUTHORIZED,
        ));
    }
}

// Handle logout
async fn handle_logout() -> Result<impl warp::Reply, warp::Rejection> {
    println!(" LOGOUT:");
    println!("   Timestamp: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!("   ─────────────────────────────");
    
    Ok(warp::reply::with_status(
        json(&AuthResponse {
            success: true,
            message: "Logged out successfully".to_string(),
            error: None,
        }),
        StatusCode::OK,
    ))
}

// Handle WebSocket connections
async fn handle_socket(ws: WebSocket, clients: Clients) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Create an unbounded mpsc channel to send messages to this specific client
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let client_key = uuid::Uuid::new_v4().to_string();

    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.insert(client_key.clone(), tx);
    }

    println!("🔗 New WebSocket connection established: {}", client_key);
    println!("   Total connected clients: {}", clients.lock().unwrap().len());

    // Task to forward messages from our internal channel to the WebSocket client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(Message::text(msg)).await.is_err() {
                break;
            }
        }
    });

    let clients_clone = clients.clone();
    let client_key_clone = client_key.clone();

    // Main loop: receive incoming WebSocket messages from this client
    while let Some(result) = ws_rx.next().await {
        if let Ok(msg) = result {
            if msg.is_text() {
                let raw_text = msg.to_str().unwrap_or_default().to_string();
                
                // Try to parse as JSON for user messages
                if let Ok(json_msg) = serde_json::from_str::<serde_json::Value>(&raw_text) {
                    if let Some(msg_type) = json_msg.get("type").and_then(|t| t.as_str()) {
                        if msg_type == "auth" {
                            if let Some(email) = json_msg.get("email").and_then(|e| e.as_str()) {
                                println!("🔐 User authenticated via WebSocket: {}", email);
                                println!("   Client ID: {}", client_key_clone);
                                println!("   ─────────────────────────────");
                            }
                        } else if msg_type == "message" {
                            if let (Some(sender), Some(text_content)) = (
                                json_msg.get("sender").and_then(|s| s.as_str()),
                                json_msg.get("text").and_then(|t| t.as_str())
                            ) {
                                println!("💬 Message from {}: {}", sender, text_content);
                                println!("   Client ID: {}", client_key_clone);
                                println!("   ─────────────────────────────");
                                
                                // Broadcast full raw JSON payload to ALL OTHER clients
                                let clients_lock = clients_clone.lock().unwrap();
                                for (key, sender_tx) in clients_lock.iter() {
                                    if key != &client_key_clone {
                                        let _ = sender_tx.send(raw_text.clone());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Plain text message (fallback)
                    println!(" Received message: {}", raw_text);
                    println!("   Client ID: {}", client_key_clone);
                    println!("   ─────────────────────────────");
                    
                    // Broadcast plain text to ALL OTHER clients
                    let clients_lock = clients_clone.lock().unwrap();
                    for (key, sender_tx) in clients_lock.iter() {
                        if key != &client_key_clone {
                            let _ = sender_tx.send(raw_text.clone());
                        }
                    }
                }
            }
        }
    }

    // Cleanup happens ONLY when ws_rx finishes (connection closed)
    send_task.abort();
    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.remove(&client_key);
        println!(" Client disconnected: {}", client_key);
        println!("   Total connected clients: {}", clients_lock.len());
        println!("   ─────────────────────────────");
    }
}
use axum::{Router, routing::get};
use std::net::SocketAddr;
use tokio::net::TcpListener; // Import TcpListener

#[tokio::main]
async fn main() {
    // Build router with two routes
    let app = Router::new()
        .route("/", get(root))
        .route("/hello", get(hello_handler)); // Add a new route

    // Address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running at http://{}", addr);

    // Bind to the address using Tokio's TcpListener
    let listener = TcpListener::bind(&addr).await.unwrap();

    // Use axum::serve to run the server
    axum::serve(listener, app) // Use axum::serve function
        .await
        .unwrap();
}

// Handler for the "/" endpoint
async fn root() -> &'static str {
    "Welcome to the root endpoint!"
}

// Handler for the "/hello" endpoint
async fn hello_handler() -> &'static str {
    "Hello from the /hello endpoint!"
}
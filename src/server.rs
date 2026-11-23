// ============================================================
//  server.rs  –  HTTP API server (axum 0.7)
// ============================================================

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::database;

pub async fn run_server(port: u16) {
    let app = Router::new()
        .route("/",       get(root))
        .route("/users",  get(users))
        .route("/system", get(system));

    let addr = format!("0.0.0.0:{port}");
    println!("[Server] 🌐 HTTP API: http://{addr}");

    let listener = TcpListener::bind(&addr).await.expect("bind ไม่สำเร็จ");
    axum::serve(listener, app).await.expect("server error");
}

async fn root() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "DigtgAI Rust" }))
}

async fn users() -> Json<Value> {
    Json(json!(database::get_all_users().await))
}

async fn system() -> Json<Value> {
    let m = database::get_memory_usage();
    Json(json!({ "rss_mb": m.rss_mb, "vms_mb": m.vms_mb }))
}

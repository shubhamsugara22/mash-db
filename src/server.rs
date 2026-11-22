//! HTTP API server. Runs the engine on one dedicated thread and feeds it SQL
//! statements over a channel, because engine state is thread-local.
//!
//! Environment variables:
//! - `MASH_API_TOKEN` (required): bearer token clients must present.
//! - `MASH_BIND` (optional): listen address, default `0.0.0.0:8080`.
//! - `MASH_DATA_DIR` (optional): directory containing `data/` and `auth.json`, default current directory.
//!
//! Endpoints:
//! - `GET /health`: no auth, returns `ok`.
//! - `POST /query` with JSON `{"sql": "..."}` and header `Authorization: Bearer <token>`.
//!   Returns `200` with `{"output": "..."}`. SQL errors are reported inside `output`.
//!   Returns `401` for a missing or wrong token, `400` for a malformed body.

use std::sync::mpsc;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::Engine;

struct Job {
    sql: String,
    reply: oneshot::Sender<String>,
}

#[derive(Clone)]
struct AppState {
    jobs: mpsc::Sender<Job>,
    token: Arc<String>,
}

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
}

#[derive(Serialize)]
struct QueryResponse {
    output: String,
}

/// Entry point for `Mash_db serve`. Blocks until the server stops.
pub fn run() {
    let token = match std::env::var("MASH_API_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            eprintln!("MASH_API_TOKEN must be set to a non-empty value");
            std::process::exit(1);
        }
    };
    let bind = std::env::var("MASH_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let data_dir = std::env::var("MASH_DATA_DIR").unwrap_or_else(|_| ".".to_string());

    // Engine::open uses relative paths (data/, auth.json), so switch directory first.
    if let Err(error) = std::env::set_current_dir(&data_dir) {
        eprintln!("Cannot use MASH_DATA_DIR '{}': {}", data_dir, error);
        std::process::exit(1);
    }

    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let engine_thread = std::thread::spawn(move || {
        let mut engine = match Engine::open() {
            Ok(engine) => {
                let _ = ready_tx.send(Ok(()));
                engine
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return;
            }
        };
        while let Ok(job) = job_rx.recv() {
            let output = engine.execute_line(&job.sql);
            let _ = job.reply.send(output);
        }
        // All senders dropped: server stopped. Run normal shutdown (checkpoint, rollback notice).
        engine.shutdown();
    });

    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            eprintln!("Failed to open database: {}", error);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Engine thread exited during startup");
            std::process::exit(1);
        }
    }

    let state = AppState {
        jobs: job_tx,
        token: Arc::new(token),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/query", post(query))
        .with_state(state);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            eprintln!("Failed to start async runtime: {}", error);
            std::process::exit(1);
        }
    };

    runtime.block_on(async move {
        let listener = match tokio::net::TcpListener::bind(&bind).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("Cannot bind to {}: {}", bind, error);
                std::process::exit(1);
            }
        };
        println!("Mash_db API listening on {}", bind);
        let served = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
                println!("Shutting down...");
            })
            .await;
        if let Err(error) = served {
            eprintln!("Server error: {}", error);
        }
    });

    // The router (and its job sender) was consumed by serve, so the engine thread
    // sees the channel close, runs shutdown, and exits.
    let _ = engine_thread.join();
}

async fn health() -> &'static str {
    "ok"
}

async fn query(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<QueryResponse>, (StatusCode, &'static str)> {
    if !authorized(&headers, &state.token) {
        return Err((StatusCode::UNAUTHORIZED, "invalid or missing bearer token"));
    }

    let request: QueryRequest = serde_json::from_str(&body).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "body must be JSON like {\"sql\": \"...\"}",
        )
    })?;

    let (reply_tx, reply_rx) = oneshot::channel();
    state
        .jobs
        .send(Job {
            sql: request.sql,
            reply: reply_tx,
        })
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "engine is not running"))?;

    let output = reply_rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "engine dropped the request",
        )
    })?;

    Ok(Json(QueryResponse { output }))
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let Some(value) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(presented) = value.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_eq(presented.as_bytes(), token.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

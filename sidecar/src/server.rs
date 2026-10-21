//! Loopback HTTP API used by the Hermes plugin.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::calls::{Bridge, Target};
use crate::config::digits;

type ApiResult = Result<Response, (StatusCode, Json<serde_json::Value>)>;

fn error(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(json!({"error": msg.into()})))
}

fn authorize(
    bridge: &Bridge,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let expected = format!("Bearer {}", bridge.cfg.token);
    match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(got) if constant_time_eq(got.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => Err(error(StatusCode::UNAUTHORIZED, "bad or missing token")),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn health(State(bridge): State<Arc<Bridge>>, headers: HeaderMap) -> ApiResult {
    authorize(&bridge, &headers)?;
    Ok(Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "logged_in": bridge.client.is_logged_in(),
        "active_call": bridge.calls.active(),
        "hermes_connected": !bridge.cfg.hermes_api_key.is_empty(),
    }))
    .into_response())
}

#[derive(Deserialize)]
struct StartCall {
    reason: String,
    #[serde(default)]
    details: String,
    /// Someone other than the owner to call (international number). Omit to call the owner.
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    contact_name: Option<String>,
    /// Language code for the callee, e.g. ru-RU.
    #[serde(default)]
    language: Option<String>,
}

async fn start_call(
    State(bridge): State<Arc<Bridge>>,
    headers: HeaderMap,
    Json(body): Json<StartCall>,
) -> ApiResult {
    authorize(&bridge, &headers)?;
    let reason = body.reason.trim().to_string();
    if reason.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "reason is required"));
    }
    if !bridge.client.is_logged_in() {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "not connected to WhatsApp",
        ));
    }
    let target = match body.to.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        None => None,
        Some(raw) => {
            let number = digits(raw);
            if number.len() < 8 || number.len() > 15 {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "`to` must be a full international phone number, e.g. +905551234567",
                ));
            }
            Some(Target {
                name: body
                    .contact_name
                    .clone()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                language: body
                    .language
                    .clone()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| contact_language(&number)),
                number,
            })
        }
    };
    let record = bridge
        .begin_outbound(&reason, target.as_ref())
        .map_err(|e| error(StatusCode::CONFLICT, e.to_string()))?;
    tokio::spawn(bridge.clone().run_outbound(
        record.id.clone(),
        reason,
        body.details.trim().to_string(),
        target,
    ));
    Ok((StatusCode::ACCEPTED, Json(record)).into_response())
}

/// The saved language for a number in the plugin's address book (contacts.json), if any.
fn contact_language(number: &str) -> String {
    let Ok(dir) = crate::config::Config::data_dir() else {
        return String::new();
    };
    let Ok(raw) = std::fs::read_to_string(dir.join("contacts.json")) else {
        return String::new();
    };
    let Ok(book) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    book.as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .find(|e| e["number"].as_str() == Some(number))
        .and_then(|e| e["language"].as_str())
        .map(|l| l.trim().to_string())
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<usize>,
}

async fn list_calls(
    State(bridge): State<Arc<Bridge>>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> ApiResult {
    authorize(&bridge, &headers)?;
    let calls = bridge.calls.recent(q.limit.unwrap_or(5).min(50));
    Ok(Json(json!({"calls": calls})).into_response())
}

async fn get_call(
    State(bridge): State<Arc<Bridge>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    authorize(&bridge, &headers)?;
    match bridge.calls.get(&id) {
        Some(record) => Ok(Json(record).into_response()),
        None => Err(error(StatusCode::NOT_FOUND, "no such call")),
    }
}

pub async fn serve(bridge: Arc<Bridge>) -> anyhow::Result<()> {
    let addr = bridge.cfg.listen.clone();
    let app = Router::new()
        .route("/health", get(health))
        .route("/calls", get(list_calls).post(start_call))
        .route("/calls/{id}", get(get_call))
        .with_state(bridge);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::info!("API listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

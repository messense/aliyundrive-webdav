use std::env;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Json, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use reqwest::Client;
use serde::Deserialize;
use tokio;

#[derive(Deserialize)]
struct QrCodeRequest {
    scopes: Vec<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize)]
struct AuthorizationRequest {
    grant_type: String,
    code: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Clone)]
struct AppState {
    client: Client,
}

#[tokio::main]
async fn main() {
    // Create a shared reqwest client
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    
    // Create the application state
    let state = AppState { client };
    
    let app = Router::new()
        .route("/oauth/authorize/qrcode", post(qrcode))
        .route("/oauth/access_token", post(access_token))
        .with_state(state);

    let addr = "0.0.0.0:8080";
    println!("Server running on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn qrcode(
    State(state): State<AppState>,
    Json(payload): Json<QrCodeRequest>
) -> Result<impl IntoResponse, StatusCode> {
    let client_id = env::var("ALIYUNDRIVE_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("ALIYUNDRIVE_CLIENT_SECRET").unwrap_or_default();

    let client = &state.client;
    match client
        .post("https://openapi.aliyundrive.com/oauth/authorize/qrcode")
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "scopes": payload.scopes,
            "width": payload.width,
            "height": payload.height,
        }))
        .send()
        .await
    {
        Ok(res) => {
            let status = res.status();
            let headers = res.headers().clone();
            let content_type = headers
                .get("content-type")
                .unwrap_or(&HeaderValue::from_static("application/json"))
                .to_str()
                .unwrap_or("application/json")
                .to_string();

            let body = res.bytes().await.unwrap_or_default();
            Ok(Response::builder()
                .status(status)
                .header("Content-Type", content_type)
                .body(Body::from(body))
                .unwrap())
        }
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn access_token(
    State(state): State<AppState>,
    Json(payload): Json<AuthorizationRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if payload.code.is_none() && payload.refresh_token.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let client_id = env::var("ALIYUNDRIVE_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("ALIYUNDRIVE_CLIENT_SECRET").unwrap_or_default();

    let client = &state.client;
    match client
        .post("https://openapi.aliyundrive.com/oauth/access_token")
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": payload.grant_type,
            "code": payload.code,
            "refresh_token": payload.refresh_token,
        }))
        .send()
        .await
    {
        Ok(res) => {
            let status = res.status();
            let headers = res.headers().clone();
            let content_type = headers
                .get("content-type")
                .unwrap_or(&HeaderValue::from_static("application/json"))
                .to_str()
                .unwrap_or("application/json")
                .to_string();

            let body = res.bytes().await.unwrap_or_default();
            Ok(Response::builder()
                .status(status)
                .header("Content-Type", content_type)
                .body(Body::from(body))
                .unwrap())
        }
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

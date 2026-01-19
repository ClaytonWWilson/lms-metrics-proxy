use axum::{
    body::Body,
    extract::{Request, State},
    http::HeaderMap,
    response::Response,
};
use bytes::Bytes;
use chrono::Utc;
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio_stream::StreamExt;

use crate::config::Config;
use crate::db::RequestRecord;
use crate::error::ProxyError;
use crate::proxy::client::HttpClient;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub client: HttpClient,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    model: Option<String>,
    messages: Option<Vec<Value>>,
    prompt: Option<String>,
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<Message>,
    text: Option<String>,
    delta: Option<Delta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct Usage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Result<Response, ProxyError> {
    let start_time = Utc::now();
    let endpoint = req.uri().path().to_string();
    let method = req.method().clone();

    // Extract the request body
    let (parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|e| ProxyError::Http(e.to_string()))?
        .to_bytes();

    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

    // For GET requests or other methods without a body, just proxy through without tracking
    // Only track POST requests that create completions/chat completions
    if method != "POST" || body_str.is_empty() {
        return simple_proxy(state, parts, body_str, method).await;
    }

    // Parse the request to check if it's streaming
    let chat_req: ChatRequest = serde_json::from_str(&body_str).unwrap_or(ChatRequest {
        model: None,
        messages: None,
        prompt: None,
        stream: Some(false),
    });

    let model = chat_req
        .model
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let is_streaming = chat_req.stream.unwrap_or(false);

    // Extract prompt from messages or prompt field
    let prompt_str = if let Some(messages) = &chat_req.messages {
        serde_json::to_string(messages).unwrap_or_default()
    } else if let Some(prompt) = &chat_req.prompt {
        prompt.clone()
    } else {
        body_str.clone()
    };

    // Create request record
    let mut record = RequestRecord::new(endpoint.clone(), model.clone(), start_time, prompt_str);

    // Reconstruct the request
    let mut hyper_req = hyper::Request::builder()
        .method(parts.method.clone())
        .uri(parts.uri.clone())
        .body(body_str.clone())
        .map_err(|e| ProxyError::Http(e.to_string()))?;

    // Copy headers
    *hyper_req.headers_mut() = parts.headers.clone();

    // Forward request to LM Studio
    let lm_response = crate::proxy::client::forward_request(
        &state.client,
        hyper_req,
        &state.config.lm_studio_url,
    )
    .await;

    match lm_response {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();

            if is_streaming && status.is_success() {
                // Handle streaming response
                handle_streaming_response(state, record, response, headers).await
            } else {
                // Handle non-streaming response
                handle_non_streaming_response(state, record, response).await
            }
        }
        Err(e) => {
            // Log error to database
            let end_time = Utc::now();
            record.set_error(end_time, e.to_string(), 502);

            if let Err(db_err) = crate::db::insert_request(&state.db, &record).await {
                tracing::error!("Failed to log error to database: {}", db_err);
            }

            Err(e)
        }
    }
}

async fn handle_non_streaming_response(
    state: Arc<AppState>,
    mut record: RequestRecord,
    response: hyper::Response<hyper::body::Incoming>,
) -> Result<Response, ProxyError> {
    let status = response.status();
    let headers = response.headers().clone();

    // Collect the response body
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| ProxyError::Http(e.to_string()))?
        .to_bytes();

    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    let end_time = Utc::now();

    // Parse the response to extract token usage
    if status.is_success() {
        if let Ok(chat_response) = serde_json::from_str::<ChatResponse>(&body_str) {
            let output = extract_output(&chat_response);
            let input_tokens = chat_response
                .usage
                .as_ref()
                .and_then(|u| u.prompt_tokens)
                .unwrap_or(0);
            let output_tokens = chat_response
                .usage
                .as_ref()
                .and_then(|u| u.completion_tokens)
                .unwrap_or(0);

            record.complete(
                end_time,
                output,
                input_tokens,
                output_tokens,
                status.as_u16() as i32,
                false,
            );

            if let Some(id) = chat_response.id {
                record.request_id = Some(id);
            }
        } else {
            record.set_error(end_time, "Failed to parse response".to_string(), status.as_u16() as i32);
        }
    } else {
        record.set_error(end_time, body_str.clone(), status.as_u16() as i32);
    }

    // Log to database (don't fail if this errors)
    if let Err(e) = crate::db::insert_request(&state.db, &record).await {
        tracing::error!("Failed to log request to database: {}", e);
    }

    // Build and return response
    let mut response_builder = Response::builder().status(status);
    for (key, value) in headers.iter() {
        response_builder = response_builder.header(key, value);
    }

    Ok(response_builder
        .body(Body::from(body_bytes))
        .map_err(|e| ProxyError::Http(e.to_string()))?)
}

async fn handle_streaming_response(
    state: Arc<AppState>,
    mut record: RequestRecord,
    response: hyper::Response<hyper::body::Incoming>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let status = response.status();

    // Create a channel for streaming to client
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::io::Error>>(100);

    // Spawn a task to process the stream
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut buffer = String::new();
        let mut last_usage: Option<Usage> = None;
        let mut request_id: Option<String> = None;

        let body_stream = response.into_body();
        let mut frame_stream = http_body_util::BodyStream::new(body_stream);

        while let Some(frame_result) = frame_stream.next().await {
            match frame_result {
                Ok(frame) => {
                    if let Ok(data) = frame.into_data() {
                        let chunk = String::from_utf8_lossy(&data).to_string();

                        // Forward to client immediately
                        if tx.send(Ok(chunk.clone())).await.is_err() {
                            tracing::warn!("Client disconnected during streaming");
                            break;
                        }

                        // Parse SSE chunks
                        for line in chunk.lines() {
                            if let Some(json_str) = line.strip_prefix("data: ") {
                                if json_str == "[DONE]" {
                                    continue;
                                }

                                if let Ok(chunk_data) = serde_json::from_str::<Value>(json_str) {
                                    // Extract request ID
                                    if let Some(id) = chunk_data.get("id").and_then(|v| v.as_str()) {
                                        request_id = Some(id.to_string());
                                    }

                                    // Extract content delta
                                    if let Some(choices) = chunk_data.get("choices").and_then(|v| v.as_array()) {
                                        if let Some(choice) = choices.first() {
                                            if let Some(delta) = choice.get("delta") {
                                                if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                                    buffer.push_str(content);
                                                }
                                            }
                                        }
                                    }

                                    // Extract usage (usually in last chunk)
                                    if let Some(usage) = chunk_data.get("usage") {
                                        if let Ok(usage_data) = serde_json::from_value::<Usage>(usage.clone()) {
                                            last_usage = Some(usage_data);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading stream: {}", e);
                    break;
                }
            }
        }

        // Stream complete - log to database
        let end_time = Utc::now();
        let input_tokens = last_usage.as_ref().and_then(|u| u.prompt_tokens).unwrap_or(0);
        let output_tokens = last_usage.as_ref().and_then(|u| u.completion_tokens).unwrap_or(0);

        record.complete(
            end_time,
            buffer,
            input_tokens,
            output_tokens,
            status.as_u16() as i32,
            true,
        );

        if let Some(id) = request_id {
            record.request_id = Some(id);
        }

        if let Err(e) = crate::db::insert_request(&state_clone.db, &record).await {
            tracing::error!("Failed to log streaming request to database: {}", e);
        }
    });

    // Convert receiver to SSE stream
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    // Build response with SSE headers
    let mut response_builder = Response::builder()
        .status(status)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive");

    // Copy other headers
    for (key, value) in headers.iter() {
        if key != "content-type" && key != "cache-control" && key != "connection" {
            response_builder = response_builder.header(key, value);
        }
    }

    // Convert stream to Body
    let body = Body::from_stream(stream.map(|result| {
        result.map(|s| Bytes::from(s))
    }));

    Ok(response_builder
        .body(body)
        .map_err(|e| ProxyError::Http(e.to_string()))?)
}

async fn simple_proxy(
    state: Arc<AppState>,
    parts: axum::http::request::Parts,
    body_str: String,
    method: axum::http::Method,
) -> Result<Response, ProxyError> {
    // Reconstruct the request for simple proxying (GET, DELETE, etc.)
    let mut hyper_req = hyper::Request::builder()
        .method(method)
        .uri(parts.uri.clone())
        .body(body_str)
        .map_err(|e| ProxyError::Http(e.to_string()))?;

    // Copy headers
    *hyper_req.headers_mut() = parts.headers.clone();

    // Forward to LM Studio
    let lm_response = crate::proxy::client::forward_request(
        &state.client,
        hyper_req,
        &state.config.lm_studio_url,
    )
    .await?;

    let status = lm_response.status();
    let headers = lm_response.headers().clone();

    // Collect response body
    let body_bytes = lm_response
        .into_body()
        .collect()
        .await
        .map_err(|e| ProxyError::Http(e.to_string()))?
        .to_bytes();

    // Build and return response
    let mut response_builder = Response::builder().status(status);
    for (key, value) in headers.iter() {
        response_builder = response_builder.header(key, value);
    }

    Ok(response_builder
        .body(Body::from(body_bytes))
        .map_err(|e| ProxyError::Http(e.to_string()))?)
}

fn extract_output(response: &ChatResponse) -> String {
    if let Some(first_choice) = response.choices.first() {
        if let Some(message) = &first_choice.message {
            if let Some(content) = &message.content {
                return content.clone();
            }
        }
        if let Some(text) = &first_choice.text {
            return text.clone();
        }
    }
    String::new()
}

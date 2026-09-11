//! Soma & Souffle — serveur unique : fichiers statiques + API chat (SSE).

mod config;
mod llm;
mod rag;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use futures::StreamExt;
use rig::agent::MultiTurnStreamItem;
use rig::prelude::*;
use rig::streaming::StreamedAssistantContent;
use serde::Deserialize;
use serde_json::json;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

struct AppState {
    cfg: config::Config,
    llm: llm::LlmClient,
    rag: rag::RagIndex,
}

type SharedState = Arc<AppState>;

/// System prompt porté du projet Lovable, enrichi des consignes RAG.
const SYSTEM_PROMPT: &str = "\
You are a warm, knowledgeable assistant for \"Soma & Souffle\", a cupping \
therapy (ventouses / vacuothérapie) wellness practice in Paris, France.

Your role:
- Answer visitor questions about cupping therapy in a calm, reassuring tone.
- Explain the benefits (tension relief, improved circulation, lymphatic drainage, relaxation, athletic recovery).
- Address common concerns (does it hurt, how long marks last, contraindications, what to expect).
- Guide interested visitors toward booking a session (email: bonjour@soma-souffle.com).
- Never diagnose medical conditions or make unsupported health claims.
- If the knowledge base excerpts above answer the question, use their exact details (prices, durations, location).
- Keep responses concise and conversational.
- Respond in the same language the visitor is using (French or English).";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = config::Config::from_env()?;
    let rag = rag::RagIndex::load(&cfg.knowledge_dir)?;
    let llm = llm::LlmClient::new(&cfg)?;
    tracing::info!(
        provider = llm.provider_name(),
        model = %cfg.model,
        chunks = rag.len(),
        "Soma & Souffle — API prête"
    );

    let state: SharedState = Arc::new(AppState { cfg, llm, rag });

    // Site statique (avec gzip + repli sur index.html), servi par le même process.
    let static_root = PathBuf::from(&state.cfg.static_dir);
    let index_file = static_root.join("index.html");
    let static_svc = ServiceBuilder::new()
        .layer(CompressionLayer::new())
        .service(
            ServeDir::new(&static_root).not_found_service(ServeFile::new(index_file)),
        );

    let api = Router::new()
        .route("/health", get(health))
        .route("/chat", post(chat))
        .with_state(state.clone());

    let app = Router::new().nest("/api", api).fallback_service(static_svc);

    let port = state.cfg.port;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("écoute sur http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("installation du handler SIGINT");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installation du handler SIGTERM")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("arrêt demandé");
}

async fn health(State(state): State<SharedState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "llm": state.llm.provider_name(),
        "model": state.cfg.model,
        "knowledge_chunks": state.rag.len(),
    }))
}

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    /// Langue de l'interface ("fr" | "en") — indice pour la réponse du LLM.
    #[serde(default)]
    lang: Option<String>,
}

/// POST /api/chat — reçoit l'historique, renvoie un flux SSE :
///   data: {"delta":"..."}  (répété)
///   data: {"done":true}
///   data: {"error":"..."}  en cas d'erreur en cours de flux
async fn chat(
    State(state): State<SharedState>,
    Json(req): Json<ChatRequest>,
) -> Response {
    if req.messages.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "le champ messages est requis");
    }
    let last = req.messages.last().expect("non vide vérifié");
    if last.role != "user" {
        return error_response(
            StatusCode::BAD_REQUEST,
            "le dernier message doit avoir role=user",
        );
    }

    // Historique (sans le dernier message = le prompt du tour).
    let history: Vec<Message> = req.messages[..req.messages.len() - 1]
        .iter()
        .filter_map(|m| match m.role.as_str() {
            "user" => Some(Message::user(m.content.clone())),
            "assistant" => Some(Message::assistant(m.content.clone())),
            _ => None, // les rôles system sont gérés côté serveur
        })
        .collect();
    let prompt = last.content.clone();

    // RAG : injection des passages pertinents dans le preamble du tour.
    let mut preamble = SYSTEM_PROMPT.to_string();
    if let Some(ctx) = state.rag.format_context(&prompt, state.cfg.rag_top_k) {
        preamble.push_str(&ctx);
    }
    match req.lang.as_deref() {
        Some("en") => preamble.push_str(
            "\n\nThe visitor is browsing the English version of the site: reply in English unless they write in another language.",
        ),
        _ => preamble.push_str(
            "\n\nLe visiteur consulte la version française du site : réponds en français sauf s'il écrit dans une autre langue.",
        ),
    }

    let agent = state.llm.agent(&state.cfg.model, state.cfg.max_tokens, preamble);
    // En rig 0.42, la requête de streaming s'attend directement (IntoFuture).
    let rig_stream = agent.stream_chat(prompt, history).await;

    let sse_stream = rig_stream.filter_map(|item| {
        let event: Option<Event> = match item {
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Text(t),
            )) => Some(Event::default().data(json!({ "delta": t.text }).to_string())),
            Ok(MultiTurnStreamItem::FinalResponse(_)) => {
                Some(Event::default().data(json!({ "done": true }).to_string()))
            }
            Ok(_) => None,
            Err(e) => {
                tracing::error!("erreur de flux LLM : {e}");
                Some(Event::default().data(json!({ "error": e.to_string() }).to_string()))
            }
        };
        futures::future::ready(event.map(|e| Ok::<_, Infallible>(e)))
    });

    Sse::new(sse_stream).into_response()
}

fn error_response(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

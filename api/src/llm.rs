//! Couche LLM via Rig : provider Ollama natif ou OpenAI-compatible,
//! choisi par configuration, avec streaming multi-tours.

use rig::client::Nothing;
use rig::prelude::*;
use rig::providers::{ollama, openai};
use rig::Agent;

use crate::config::{Config, ProviderKind};

pub enum LlmClient {
    Ollama(ollama::Client),
    OpenAiCompat(openai::CompletionsClient),
}

impl LlmClient {
    pub fn new(cfg: &Config) -> anyhow::Result<Self> {
        match cfg.provider {
            ProviderKind::Ollama => {
                let client = ollama::Client::builder()
                    .api_key(Nothing)
                    .base_url(cfg.base_url.clone())
                    .build()?;
                tracing::info!(url = %cfg.base_url, "provider LLM : Ollama");
                Ok(Self::Ollama(client))
            }
            ProviderKind::OpenAiCompat => {
                // CompletionsClient = API /chat/completions, celle qu'exposent
                // tous les fournisseurs compatibles OpenAI (Groq, Mistral, OpenRouter…).
                let client = openai::CompletionsClient::builder()
                    .api_key(cfg.api_key.clone())
                    .base_url(cfg.base_url.clone())
                    .build()?;
                tracing::info!(url = %cfg.base_url, "provider LLM : OpenAI-compatible");
                Ok(Self::OpenAiCompat(client))
            }
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            LlmClient::Ollama(_) => "ollama",
            LlmClient::OpenAiCompat(_) => "openai_compat",
        }
    }

    /// Construit un agent (léger) avec le preamble de la requête —
    /// il embarque le system prompt et le contexte RAG du tour.
    pub fn agent(&self, model: &str, max_tokens: u64, preamble: String) -> Agent {
        match self {
            LlmClient::Ollama(c) => c
                .agent(model.to_string())
                .preamble(&preamble)
                .max_tokens(max_tokens)
                .build(),
            LlmClient::OpenAiCompat(c) => c
                .agent(model.to_string())
                .preamble(&preamble)
                .max_tokens(max_tokens)
                .build(),
        }
    }
}

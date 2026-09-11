//! Configuration par variables d'environnement (.env supporté).

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Ollama,
    OpenAiCompat,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Ollama => "ollama",
            ProviderKind::OpenAiCompat => "openai_compat",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub provider: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Plafond de tokens générés par réponse. Le palier gratuit de Groq
    /// limite à 1000 tokens de sortie par minute (OTPM) : rester en dessous.
    pub max_tokens: u64,
    pub rag_top_k: usize,
    pub static_dir: String,
    pub knowledge_dir: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let provider = match env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| "ollama".to_string())
            .to_lowercase()
            .as_str()
        {
            "ollama" => ProviderKind::Ollama,
            "openai_compat" | "openai" | "compat" => ProviderKind::OpenAiCompat,
            other => anyhow::bail!("LLM_PROVIDER invalide : {other} (valeurs acceptées : ollama, openai_compat)"),
        };

        let base_url = env::var("LLM_BASE_URL").unwrap_or_else(|_| match provider {
            // Dans Docker, host.docker.internal pointe vers l'Ollama de la machine hôte.
            ProviderKind::Ollama => "http://host.docker.internal:11434".to_string(),
            ProviderKind::OpenAiCompat => "https://api.openai.com/v1".to_string(),
        });

        let api_key = env::var("LLM_API_KEY").unwrap_or_default();
        if provider == ProviderKind::OpenAiCompat && api_key.is_empty() {
            anyhow::bail!("LLM_API_KEY est requis quand LLM_PROVIDER=openai_compat");
        }

        Ok(Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            provider,
            base_url,
            api_key,
            model: env::var("LLM_MODEL").unwrap_or_else(|_| "qwen2.5:3b".to_string()),
            max_tokens: env::var("LLM_MAX_TOKENS")
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or(600),
            rag_top_k: env::var("RAG_TOP_K")
                .ok()
                .and_then(|k| k.parse().ok())
                .unwrap_or(3),
            static_dir: env::var("STATIC_DIR").unwrap_or_else(|_| "./static".to_string()),
            knowledge_dir: env::var("KNOWLEDGE_DIR").unwrap_or_else(|_| "./knowledge".to_string()),
        })
    }
}

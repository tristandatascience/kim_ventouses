# Soma & Souffle — refonte légère

Refonte de la landing page « Soma & Souffle » (thérapie par ventouses à Paris)
**sans TypeScript, sans Node.js, sans framework** : HTML/CSS/JS vanilla côté
client, un unique binaire **Rust** (Axum + Rig) côté serveur, un **RAG BM25**
interne, et un **conteneur Docker unique** pour tout servir.

```
Navigateur ──► conteneur unique (port 8080)
                ├── fichiers statiques (frontend/, gzip)
                ├── POST /api/chat  ──► Rig ──► Ollama / API cloud compatible OpenAI
                └── GET  /api/health
                     └─ RAG : api/knowledge/*.md indexés en BM25 (mémoire ~0)
```

## Pourquoi cette architecture

| Choix | Raison |
|---|---|
| HTML/CSS/JS vanilla | zéro build, zéro Node, quelques Ko |
| [quikchat](https://github.com/deftio/quikchat) (vendorisé) | UI de chat vanilla avec streaming token-par-token |
| Rust + Axum | un seul process sert le statique ET l'API (~50-80 Mo de RAM) |
| [Rig](https://github.com/0xPlaygrounds/rig) | abstraction LLM : Ollama natif ou tout endpoint compatible OpenAI, streaming |
| RAG BM25 maison | aucune dépendance à un modèle d'embeddings — indispensable sur un VPS 1 Go sans Ollama |
| Conteneur unique | moins d'images, moins de RAM, moins de disque (VPS 1 Go / 10 Go) |

## Démarrage

```bash
cp .env.example .env   # puis adapter le scénario LLM (voir ci-dessous)
docker compose up -d --build
# → http://localhost:8080
```

Le chat teste l'API via `curl` :

```bash
curl -N -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"Est-ce que les ventouses font mal ?"}],"lang":"fr"}'
```

## Les 3 scénarios LLM (.env)

### 1. Dev : Ollama sur la machine hôte (défaut)
```env
LLM_PROVIDER=ollama
LLM_BASE_URL=http://host.docker.internal:11434
LLM_MODEL=qwen2.5:3b
```
Prérequis : Ollama lancé sur la machine + `ollama pull qwen2.5:3b`.

### 2. VPS ≥ 4 Go : Ollama conteneurisé
```bash
docker compose --profile ollama up -d
docker compose exec ollama ollama pull qwen2.5:3b
```
```env
LLM_PROVIDER=ollama
LLM_BASE_URL=http://ollama:11434
LLM_MODEL=qwen2.5:3b
```

### 3. VPS 1 Go (1 vCore / 10 Go) : API cloud compatible OpenAI
Un modèle 3B consomme ~2 Go de RAM : impossible ici, le LLM est donc externe.
```env
LLM_PROVIDER=openai_compat
LLM_BASE_URL=https://api.groq.com/openai/v1   # ou OpenAI, Mistral, OpenRouter…
LLM_MODEL=qwen/qwen3.8-27b
LLM_API_KEY=gsk_xxxxxxxxxxxx
```
Dans ce scénario le conteneur `app` consomme ~60 Mo de RAM — largement dans
le budget d'un VPS 1 Go (limite fixée à 150 Mo dans docker-compose.yml).

## Déploiement sur le VPS (important)

**Ne jamais compiler Rust sur le VPS 1 Go** (cargo exige bien plus d'1 Go).
Construire l'image sur la machine de dev, puis l'exporter :

```bash
# machine de dev
docker compose build
docker save soma-souffle:latest | gzip > soma-souffle.tar.gz
scp soma-souffle.tar.gz user@vps:

# VPS
gunzip -c soma-souffle.tar.gz | docker load
# copier docker-compose.yml + .env (sans build:), puis :
docker compose up -d
```

Sur le VPS, retirer la ligne `build: .` du service `app` dans
docker-compose.yml (l'image est déjà chargée).

## Base de connaissance (RAG)

Ajouter/modifier des fichiers Markdown dans `api/knowledge/` — chaque section
`## ` devient un passage indexé. BM25 (k1=1.5, b=0.75) recherche les `RAG_TOP_K`
passages les plus pertinents et les injecte dans le prompt système du tour.

Pour passer plus tard à un RAG par embeddings (si l'infra le permet) :
remplacer `RagIndex::search` dans `api/src/rag.rs` par le vector store de Rig
(`rig::vector_store::in_memory_store::InMemoryVectorStore`) — le reste du code
n'a pas besoin de changer.

## Développement sans Docker

```bash
cd api && cargo run            # sert ./static → lancer avec STATIC_DIR=../frontend
cargo test                     # tests unitaires du RAG BM25
STATIC_DIR=../frontend KNOWLEDGE_DIR=knowledge LLM_BASE_URL=http://localhost:11434 cargo run
```

## Structure

```
soma-souffle/
├── frontend/               # 100% statique — servi par le binaire Rust
│   ├── index.html          # page unique FR/EN (data-i18n)
│   ├── css/styles.css      # design oklch porté (Playfair + Inter)
│   └── js/                 # i18n.js, main.js, chat.js + vendor/quikchat/
├── api/                    # binaire soma-api
│   ├── src/main.rs         # Axum : statique + /api/health + /api/chat (SSE)
│   ├── src/config.rs       # lecture .env
│   ├── src/llm.rs          # Rig : Ollama ou OpenAI-compatible
│   ├── src/rag.rs          # BM25 maison + tests
│   └── knowledge/*.md      # base de connaissance du cabinet
├── Dockerfile              # multi-stage : rust:1-slim → debian:bookworm-slim (~85 Mo)
├── docker-compose.yml      # app (+ profil ollama optionnel)
└── .env.example
```

## Provenance

Portage fidèle de la landing page générée par Lovable (TanStack Start +
TypeScript + Supabase inutilisé) — textes FR/EN, design et system prompt du
chat conservés ;Supabase, SSR et les 49 composants shadcn inutilisés éliminés.

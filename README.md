# Ventouses & Gua Sha — site + assistant chat (RAG)

Site du cabinet (Ivry-sur-Seine) basé sur la **maquette 4 « Héritage »**
(design V04 : Fraunces + Manrope, papier/encre/terracotta) enrichi d'une couche
d'animations **GSAP + ScrollTrigger + Lenis** (révélations au défilement, rideau
photo, tuiles 3D au survol, ligne magique du menu) et d'un **assistant chat RAG**
branché sur l'API du site, le tout servi par un **unique conteneur Docker**.

```
Navigateur ──► conteneur unique (port 8080)
                ├── frontend statique (HTML/CSS/JS d'origine + quikchat vendorisé)
                ├── POST /api/chat  ──► Rig ──► API cloud compatible OpenAI (Groq)
                └── GET  /api/health
                     └─ RAG : api/knowledge/*.md indexés en BM25 (mémoire ~0)
```

## Démarrage

```bash
cp .env.example .env   # renseigner LLM_API_KEY (Groq) — voir scénarios ci-dessous
docker compose up -d --build
# → http://localhost:8080
```

Tester le chat :

```bash
curl -N -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"Quelle formule pour la nuque ?"}]}'
```

## Scénarios LLM (.env)

1. **API cloud compatible OpenAI** (défaut, adapté VPS 1 Go) : Groq, OpenAI,
   Mistral, OpenRouter… `LLM_PROVIDER=openai_compat` + `LLM_BASE_URL` +
   `LLM_API_KEY`. Palier gratuit Groq : sortie limitée à 1000 tokens/min →
   `LLM_MAX_TOKENS=600` par défaut.
2. **Ollama sur la machine hôte** (dev) : `LLM_PROVIDER=ollama`,
   `LLM_BASE_URL=http://host.docker.internal:11434`, `LLM_MODEL=qwen2.5:3b`.
3. **Ollama conteneurisé** (VPS ≥ 4 Go) : `docker compose --profile ollama up -d`
   puis `docker compose exec ollama ollama pull qwen2.5:3b`,
   `LLM_BASE_URL=http://ollama:11434`.

## Le chat

- Fenêtre « L'assistant de Kim » (dialog native de la V04) : suggestions
  cliquables, streaming token-par-token, indicateur de frappe, bouton
  « Recommencer », historique multi-tours (10 derniers messages au LLM).
- Le serveur injecte les `RAG_TOP_K` passages BM25 les plus pertinents de
  `api/knowledge/` dans le prompt système, avec consigne d'exactitude sur les
  prix et durées.

## Base de connaissance

Fichiers Markdown dans `api/knowledge/` (une section `## ` = un passage indexé) :
`cabinet.md`, `formules.md`, `deroule.md`, `faq.md`, `precautions.md`.
Modifier puis rebuild. Pour un RAG par embeddings plus tard : remplacer
`RagIndex::search` dans `api/src/rag.rs` par le vector store de Rig.

## Images du site

Les balises `<img>` du HTML d'origine pointent vers `frontend/images/*.jpg`
(lieu, ventouses, ambiance, detail, gua-sha, portrait, acces) et
**s'auto-retirent si le fichier est absent** (comportement d'origine). Déposer
de vraies photos avec ces noms suffit à les faire apparaître — aucun code à
modifier.

## Déploiement sur le VPS (1 vCore / 1 Go / 10 Go)

**Ne jamais compiler Rust sur le VPS** : construire l'image sur la machine de
dev, puis :

```bash
docker compose build
docker save soma-souffle:latest | gzip > soma-souffle.tar.gz
scp soma-souffle.tar.gz docker-compose.yml .env user@vps:
# sur le VPS : gunzip -c soma-souffle.tar.gz | docker load
# retirer la ligne build: du compose puis docker compose up -d
```

Le conteneur consomme ~6 Mo de RAM (limite 150 Mo dans le compose), l'image
fait ~140 Mo.

## Développement sans Docker

```bash
cd api
STATIC_DIR=../frontend KNOWLEDGE_DIR=knowledge LLM_BASE_URL=http://localhost:11434 cargo run
cargo test   # tests unitaires du BM25
```

## Structure

```
├── frontend/
│   ├── index.html          # design maquette 4 + animations GSAP/Lenis/jQuery (CDN)
│   ├── assets/             # favicon + photo rituel-ventouses.png
│   └── js/concierge.js     # assistant : SSE /api/chat, suggestions, historique
├── api/                    # binaire soma-api (Axum)
│   ├── src/{main,config,llm,rag}.rs
│   └── knowledge/*.md      # base de connaissance du cabinet
├── Dockerfile              # multi-stage : rust → debian-slim (~140 Mo)
└── docker-compose.yml      # app (+ profil ollama optionnel)
```

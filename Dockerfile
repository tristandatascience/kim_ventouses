# syntax=docker/dockerfile:1
# ============================================================
# Soma & Souffle — image unique (frontend statique + API Rust)
# ~85 Mo finale. Le build Rust n'est JAMAIS fait sur le VPS :
# construire sur une machine de dev puis exporter l'image.
# ============================================================

# ---- Étape 1 : compilation Rust ----
FROM rust:1-slim-bookworm AS build
WORKDIR /build

# 1a) Dépendances seules (couche cachée : ne se recompile que si
#     Cargo.toml change)
COPY api/Cargo.toml ./
RUN mkdir -p src \
 && echo 'fn main() {}' > src/main.rs \
 && cargo build --release \
 && rm -rf src \
 && rm -f target/release/deps/soma_api* target/release/soma-api

# 1b) Code réel
COPY api/src ./src
RUN touch src/main.rs && cargo build --release

# ---- Étape 2 : image finale ----
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /build/target/release/soma-api /usr/local/bin/soma-api
COPY frontend ./static
COPY api/knowledge ./knowledge

ENV PORT=8080 \
    STATIC_DIR=/app/static \
    KNOWLEDGE_DIR=/app/knowledge

EXPOSE 8080
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/soma-api"]

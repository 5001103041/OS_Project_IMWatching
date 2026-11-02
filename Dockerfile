# ==========================================
# Multi-stage Dockerfile for IMWatching
# Builds Frontend + Backend into a single image
# ==========================================

# ─── Stage 1: Build Frontend ─────────────────────────
FROM node:20-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install --frozen-lockfile || npm install
COPY frontend/ ./
RUN npm run build

# ─── Stage 2: Build Backend ──────────────────────────
FROM rust:bookworm AS backend-builder
WORKDIR /app/backend

# Pre-cache dependencies
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

# Copy real source and build release
COPY backend/ ./
RUN rm -f target/release/deps/imwatching_backend* target/release/imwatching-backend*
RUN cargo build --release

# ─── Stage 3: Runtime ────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install ffmpeg, ca-certificates, and sqlite3
RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    ca-certificates \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=backend-builder /app/backend/target/release/imwatching-backend /app/imwatching-backend

# Copy built frontend assets
COPY --from=frontend-builder /app/frontend/dist /app/dist

# Set static directory for Axum fallback
ENV STATIC_DIR=/app/dist
ENV PORT=8080
ENV MEDIA_DIR=/app/media
ENV DATABASE_URL=sqlite:///app/data/imwatching.db?mode=rwc
ENV JWT_SECRET=imwatching_default_secret_key_minimum_32_characters_for_prod_and_dev

# Create persistent storage directories
RUN mkdir -p /app/data /app/media

EXPOSE 8080

CMD ["/app/imwatching-backend"]

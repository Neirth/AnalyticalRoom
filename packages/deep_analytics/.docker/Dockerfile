# ---------- STAGE 1: Build ----------
FROM rust:1.90-slim-bullseye AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev build-essential ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .

RUN cargo build --release --bin deep_analytics

# ---------- STAGE 2: Runtime ----------
# Usamos distroless para runtime mínimo (no shell, solo binario)
FROM gcr.io/distroless/cc AS runtime

WORKDIR /app/bin

# Copiar el binario desde la etapa de build
COPY --from=builder /build/target/release/deep_analytics /app/bin/deep_analytics

# Establecer usuario no-root (distroless ya tiene nobody)
USER nonroot:nonroot

# Comando por defecto
ENTRYPOINT ["/app/bin/deep_analytics"]

# ---------- STAGE 1: Build ----------
# Usar Debian 12 (bookworm) para que compile contra libssl3
FROM rust:1.90-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev build-essential ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .

RUN cargo build --release --bin logical_engine

# ---------- STAGE 2: Runtime ----------
# Usamos distroless para runtime mínimo (no shell, solo binario)
# Por defecto usa Debian 12 (bookworm) con libssl3
FROM gcr.io/distroless/cc AS runtime

WORKDIR /app/bin

# Copiar el binario desde la etapa de build
COPY --from=builder /build/target/release/logical_engine /app/bin/logical_engine

# Establecer usuario no-root (distroless ya tiene nobody)
USER nonroot:nonroot

# Comando por defecto
ENTRYPOINT ["/app/bin/logical_engine"]

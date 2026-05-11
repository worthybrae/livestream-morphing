# Build stage
FROM rust:1.83-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libswscale-dev \
    libavfilter-dev \
    libavdevice-dev \
    pkg-config \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -f target/release/deps/livestream_morphing_rs*

COPY src/ src/
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libavcodec59 \
    libavformat59 \
    libavutil57 \
    libswscale6 \
    libavfilter8 \
    libavdevice59 \
    libx264-164 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN adduser --system --no-create-home --shell /sbin/nologin morphing

COPY --from=builder /app/target/release/livestream-morphing-rs /usr/local/bin/morphing

USER morphing
EXPOSE 8080
ENV PORT=8080
CMD ["morphing"]

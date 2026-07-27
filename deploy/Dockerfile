# Oxipage 컨테이너 이미지 (doc/05 §5.1).
# OCI 표준 → Apple container build / docker build 모두 사용 가능.
# 다단계: 1) web 빌드(bun) 2) Rust 빌드(web/dist 내장) 3) 런타임.

# ── 1단계: 프론트엔드 빌드 ──
FROM oven/bun:1 AS web-builder
WORKDIR /web
COPY web/package.json web/bun.lock* ./
RUN bun install --frozen-lockfile || bun install
COPY web/ ./
RUN bun run build

# ── 2단계: Rust 빌드 ──
FROM rust:1-bookworm AS rust-builder
WORKDIR /src
# rust-embed 컴파일타임에 web/dist 필요 — 먼저 복사.
COPY --from=web-builder /web/dist ./web/dist
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
# 바이너리명은 oxipage-server ([[bin]] name 참조). macOS 27 strip 제약은 Linux 빌드에
# 무관하지만 workspace profile(strip=none)이 그대로 적용된다.
RUN cargo build --release --bin oxipage-server

# ── 3단계: 런타임 ──
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates tzdata && rm -rf /var/lib/apt/lists/*
WORKDIR /srv/oxipage
COPY --from=rust-builder /src/target/release/oxipage-server /usr/local/bin/oxipage-server
COPY oxipage.toml.example ./oxipage.toml

# 데이터 볼륨(SQLite db, 미디어)은 /data 로 마운트.
VOLUME ["/srv/oxipage/data"]
EXPOSE 8787
ENV RUST_LOG=info
ENV OXIPAGE_CONFIG=/srv/oxipage/oxipage.toml

# 서버 바이너리는 서브커맨드 없이 직접 기동한다(인자 무시). oxipage.toml + 환경변수로 설정.
CMD ["oxipage-server"]

# Oxipage

개인 창작 작업실 — 개발자·작가·비평가·큐레이터로서의 "나"를 한곳에 모으는 셀프호스팅 개인 홈페이지.
설계 문서: `doc/00-overview.md` ~ `doc/06-roadmap.md`.

## 요구 사항

- Rust 1.96+ (stable)
- bun 1.3+ (프론트엔드 빌드 전용 — 런타임에는 Node 불필요)

## 빌드 & 실행

```bash
cd web && bun install && bun run build && cd ..   # 프론트엔드 빌드 (바이너리에 임베드)
cargo build --release -p oxipage-server            # → target/release/oxipage-core
OXIPAGE_ADMIN_TOKEN=<랜덤 토큰> ./target/release/oxipage-core
# http://127.0.0.1:8787
```

- 설정: `oxipage.toml` (없으면 기본값으로 기동). `OXIPAGE_CONFIG`, `OXIPAGE_PORT`, `OXIPAGE_DATA_DIR` 환경변수로 오버라이드.
- 쓰기 API: `Authorization: Bearer $OXIPAGE_ADMIN_TOKEN` (v0 임시 인증; PAT 체계는 로드맵 Phase 1/4).

> **macOS 27 참고:** release 프로필에 `strip = "none"`을 고정해 두었다. macOS 27의 dyld가
> strip된 Mach-O dylib의 mis-aligned string pool을 거부해(rust-lang/rust#157750)
> 기본값(`debuginfo`) 그대로 빌드하면 proc-macro 로딩이 실패한다.

## 개발 워크플로우

```bash
cargo run -p oxipage-server     # 백엔드 :8787 (debug 빌드는 web/dist를 디스크에서 읽음)
cd web && bun run dev           # 프론트엔드 개발 서버 :5173 (/api → :8787 프록시)
```

## 테스트

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

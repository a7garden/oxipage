# WASM 런타임 적재 v2 (doc/01 §1.4, doc/08 §8.4)

> **상태:** v2 완료 (2026-07-28). v1 스파이크의 한계 3종(store 재사용, HTTP 라우트,
> 핫 리로드)를 해결하고 §7의 fuel 제한·DB/HTTP capability·서명 검증을 구현했다.
> 컴포넌트 모델(WIT/wasip2)은 환경 제약(`cargo-component`/`wit-bindgen` 미설치)으로 남겨둔 과제.

## 1. 무엇을 증명했는가

### v1 (스파이크, 2026-07-28 초)

1. wasmtime 임베딩 — Rust 호스트가 `.wasm` 코어 모듈을 인스턴스화해 `Extension` trait으로 노출.
2. 수동 ABI — 모듈이 export 한 `memory` + ptr/len 함수로 호스트가 UTF-8/JSON 문자열을 읽는다.
3. capability-based 샌드박스 — 모듈이 호스트 함수를 import 로 호출. WASM은 자체 I/O가 없으므로
   호스트가 부여한 capability만 접근 가능.
4. `Extension` 서브셋 미러링 — `id`/`display_name`/`lobby_summary`만 WASM 경계를 넘었다.
5. 런타임 설치 — `POST /extensions/install` 이 레지스트리에서 `.wasm`을 저장 + `extension_state` 행 추가.
6. 부팅 시 적재 — `--features wasm` 서버가 `data/extensions/*.wasm`을 스캔해 정적 목록에 추가.

### v2 (2026-07-28, 본 세션)

7. **Store 재사용** — lobby Store+Instance를 `Mutex`로 캐싱. 매 호출 재인스턴스화 제거 (v1 한계 #5 해결).
8. **Fuel 제한** — `Config::consume_fuel(true)` + `Store::set_fuel(N)`. 라우트 요청당 10M, lobby 1M.
   CPU 과다로 인한 DoS 방지 (§7 #4 구현).
9. **HTTP 라우트** — route manifest ABI + `handle_request` export. 코어 `RouteDispatcher` trait 구현.
   폴백 핸들러가 요청 시점에 동적 디스패치 (v1 한계 #1 해결).
10. **DB/HTTP capability** — `host_db_query(SELECT ...)` (read-only), `host_http_get(url)` host import
    추가. capability 링커에 등록 (§7 #2 구현).
11. **핫 리로드** — `ExtensionRegistry`가 `std::sync::RwLock<Vec<Arc<dyn Extension>>>`로 확장 목록 보호.
    install 엔드포인트가 `WasmLoader` trait으로 라이브 활성화 (재기동 불필요) (v1 한계 #4 + §7 #3 해결).
12. **서명 검증** — ed25519 서명(`ed25519-dalek`) 검증. registry `index.json`의 `signature` 필드로
    `.wasm` 바이트 위변조 탐지 (§7 #5 구현).

## 2. 아키텍처 (v2)

```
┌─────────────────────────────────────────────────────────────────┐
│ oxipage-server (feature "wasm")                                 │
│   run_server_with_extensions:                                   │
│     all = static_extensions()                                   │
│     all.extend(oxipage_wasm::load_all_from_dir(ext_dir))       │
│   → ExtensionRegistry (RwLock<Vec>, 핫 리로드 가능)             │
│   build_app:                                                    │
│     컴파일 확장 → nest(id, routes())                            │
│     WASM 확장 → route_dispatcher()가 Some → 네스팅 스킵          │
│     fallback → api_fallback (동적 디스패치)                     │
│   install 엔드포인트:                                            │
│     1. ed25519 서명 검증 (signature 있을 시)                    │
│     2. wasm_loader.load(path) → registry.register_and_activate()│
│     → 즉시 활성화 (재기동 불필요)                                │
└──────────────────────────────┬──────────────────────────────────┘
                               │ loads .wasm
┌──────────────────────────────▼──────────────────────────────────┐
│ oxipage-wasm v2  (WasmExtensionAdapter: impl Extension + RouteDispatcher) │
│   Engine (consume_fuel=true) + Module (1회 컴파일, 공유)        │
│   Mutex<LobbyCache> { Store + Instance } — lobby 카드 재사용    │
│   dispatch_request: fresh Store per request (fuel isolation)    │
│   build_linker(): import "env" {                                │
│     host_now_unix, host_log, host_db_query, host_http_get       │
│   }                                                             │
│   WasmLoaderImpl: impl WasmLoader (서버가 주입)                 │
└──────────────────────────────┬──────────────────────────────────┘
                               │ core wasm ABI v2
┌──────────────────────────────▼──────────────────────────────────┐
│ oxipage-ext-wasm-demo v2  (cdylib, no_std, wasm32)              │
│   export: memory, init, ext_id_*, display_name_*,               │
│           lobby_card_*,                                         │
│           alloc, reset_alloc (bump allocator),                  │
│           route_manifest_* (JSON 라우트 목록),                   │
│           handle_request, handle_request_len                    │
│   routes: GET /info, GET /time, GET /db                         │
│   import: env::{host_now_unix, host_log, host_db_query, host_http_get} │
└─────────────────────────────────────────────────────────────────┘
```

## 3. ABI 명세 (v2)

### 모듈 export (코어 WASM)

| export | 시그니처 | 비고 |
|---|---|---|
| `memory` | linear memory | `--export-memory` |
| `init` | `() -> ()` | load 직후 1회; capability 호출 + lobby 버퍼 구축 |
| `ext_id_ptr` / `ext_id_len` | `() -> i32` | UTF-8 id |
| `display_name_ptr` / `display_name_len` | `(i32 lang) -> i32` | `0`=ko, `1`=en |
| `lobby_card_ptr` / `lobby_card_len` | `() -> i32` | JSON; 빈 문자열 = 카드 없음 |
| `alloc` | `(i32 size) -> i32 ptr` | linear memory bump 할당. 0 = OOM |
| `reset_alloc` | `() -> ()` | 할당자 리셋 (요청 전 호스트가 호출) |
| `route_manifest_ptr` / `route_manifest_len` | `() -> i32` | JSON `[{"method":"GET","path":"/info"},...]` |
| `handle_request` | `(i32 method, i32 path_ptr, i32 path_len, i32 body_ptr, i32 body_len) -> i64` | `(status << 32) \| resp_ptr` |
| `handle_request_len` | `() -> i32` | 응답 본문 길이 |

method 코드: 0=GET, 1=POST, 2=PUT, 3=DELETE, 4=PATCH

### 모듈 import (module = `"env"`)

| import | 시그니처 | capability |
|---|---|---|
| `host_now_unix` | `() -> i64` | read-only 시계 (epoch 초) |
| `host_log` | `(i32 ptr, i32 len) -> ()` | tracing 로깅 |
| `host_db_query` | `(i32 sql_ptr, i32 sql_len) -> i64` | read-only SQL → JSON. SELECT만 허용 |
| `host_http_get` | `(i32 url_ptr, i32 url_len) -> i64` | HTTP GET → body bytes |

`host_db_query` / `host_http_get` 반환값: `(result_ptr | (result_len << 32))` as i64.
0 = 에러. 호스트가 모듈의 `alloc(size)`으로 결과 버퍼를 할당해 쓰고 packed ptr+len 반환.

### 서명 검증

- registry `index.json`의 `signature` 필드 (base64 ed25519 서명).
- 신뢰 공개키는 바이너리에 컴파일 (`TRUSTED_WASM_PUBKEY_B64`).
- install 시: `signature` 필드가 있으면 `verify_wasm_signature(bytes, sig)` 검증.
  위변조 시 `409 CONFLICT signature_mismatch`.
- `signature` 필드가 없으면 검증 생략 (향후 strict mode 추가 가능).

## 4. 해결된 v1 한계

| v1 한계 | v2 해결 | 방법 |
|---|---|---|
| **HTTP 라우트 없음** | ✅ 해결 | route manifest ABI + `RouteDispatcher` trait + 폴백 디스패처 |
| **DB 마이그레이션 없음** | 제약 유지 | `Router`/`Migration`은 호스트 구체 타입. DB 쿼리는 capability로 우회 가능 |
| **CLI 서브커맨드 없음** | 제약 유지 | clap 정적 링크 필요. install 자체는 코어 CLI로 동작 |
| **부팅 시에만 적재** | ✅ 해결 | `RwLock<Vec>` + `register_and_activate()` + `WasmLoader` trait |
| **매 호출 store 인스턴스화** | ✅ 해결 | lobby Store+Instance `Mutex` 캐싱, 라우트는 fresh store (fuel isolation) |

## 5. §7 진척도

| §7 항목 | 상태 | 비고 |
|---|---|---|
| 컴포넌트 모델 (WIT/wasip2) | ❌ 환경 제약 | `cargo-component`/`wit-bindgen` 미설치 |
| DB/HTTP capability | ✅ 구현 | `host_db_query`(SELECT 한정), `host_http_get` |
| 핫 리로드 | ✅ 구현 | `WasmLoader` + `register_and_activate()` |
| Fuel 제한 | ✅ 구현 | 10M/request, 1M/lobby. 메모리 상한은 미구현 |
| 서명 검증 | ✅ 구현 | ed25519 (`ed25519-dalek`). registry `signature` 필드 |
| 원격 레지스트리 | 부분 | `wasm_url` 다운로드 경로 존재. GitHub 호스팅은 미구현 |

## 6. 빌드 / 실행 / 검증

### 데모 `.wasm` 빌드 + 서명

```sh
cargo build -p oxipage-ext-wasm-demo --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/oxipage_ext_wasm_demo.wasm \
   crates/oxipage-ext-wasm-demo/artifacts/wasm-demo.wasm
# 서명은 registry/index.json 의 signature 필드에 사전 기록됨.
```

### 호스트 단위 테스트 (6 tests)

```sh
cargo test -p oxipage-wasm
# loads_demo_and_extracts_static_metadata — id/display_name 추출
# lobby_summary_returns_a_card — 동적 lobby JSON round-trip
# route_manifest_extracted — route manifest 파싱
# dispatch_returns_response — GET /info → 200 + body
# store_reuse_for_lobby — 캐싱된 store 재사용 (2회 호출)
# missing_dir_is_empty_not_error — 로더 복원력
```

### install round-trip (서명 검증 + 파일 쓰기 + DB)

```sh
cargo test -p oxipage-core --test http_app install_writes_wasm
# POST /extensions/install → ed25519 서명 검증 → 200, .wasm 파일, extension_state
```

### 품질 게이트

```sh
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo clippy -p oxipage-server --features wasm -- -D warnings  # clean
cargo test --workspace                                   # backup 테스트(사전 존재 이슈) 제외 통과
```

## 7. 남은 과제 (프로덕션 하드닝)

1. **컴포넌트 모델(WIT/wasip2) 마이그레이션.** `cargo-component` + `wit-bindgen`으로 ABI를
   WIT 인터페이스로 승격. ptr/len 수동 직렬화 제거.
2. **capability 권한 모델.** per-extension capability 토큰으로 화이트리스트 테이블/도메인 제한.
3. **메모리 상한.** `Store::limiter()` 또는 `ResourceLimiter` trait으로 linear memory 최대 크기 제한.
4. **서명 키 관리.** 프로덕션에서는 공개키를 config로 주입. 다중 신뢰 키 지원.
5. **원격 레지스트리.** `registry/index.json`을 GitHub 호스팅으로 옮기고 `wasm_url`을 릴리스 자산으로.
6. **핫 언로드.** 현재는 핫 리로드만 지원. 런타임 제거(uninstall) 시 registry에서 제거 + 라우트 갱신.

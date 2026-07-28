# WASM 런타임 적재 스파이크 (doc/01 §1.4, doc/08 §8.4)

> **상태:** 스파이크 완료 (2026-07-28). v1 기능이 아님 — 가능성 탐색(proof of concept).
> doc/01 §1.4 가 언급한 "WASM 컴포넌트 기반 런타임 로딩"의 핵심 메커니즘을 코어
> WASM 모듈 경로로 증명했다. 컴포넌트 모델(WIT/wasip2) 하드닝은 다음 단계.

## 1. 무엇을 증명했는가

1. **wasmtime 임베딩** — Rust 호스트(`oxipage-wasm`)가 `.wasm` 코어 모듈을
   인스턴스화해 `Extension` trait으로 노출한다.
2. **수동 ABI** — 모듈이 export 한 `memory` + ptr/len 함수로 호스트가 UTF-8/JSON
   문자열을 읽는다. 문자열 직렬화를 위한 힙 할당 없이 정적 버퍼만으로 동작.
3. **capability-based 샌드박스** — 모듈이 호스트 함수(`host_now_unix`, `host_log`)를
   `import`로 호출한다. WASM 은 자체 I/O 가 없으므로 호스트가 부여한 capability 만
   접근 가능 → 샌드박스 경계가 명확.
4. **`Extension` 서브셋 미러링** — `id`/`display_name`/`lobby_summary`/`on_startup`만
   WASM 경계를 넘는다. 라우트·마이그레이션·백그라운드 잡은 넘지 못한다 (아래 §4).
5. **런타임 설치** — `oxipage extension install <name>` → `POST /extensions/install`
   이 `registry/index.json` 에서 `runtime_loadable: true` 항목을 찾아 `.wasm` 을
   `data/extensions/<name>.wasm` 에 저장 + `extension_state` 행 추가.
6. **부팅 시 적재** — `oxipage-server`(`--features wasm`) 가 `data/extensions/*.wasm`
   을 스캔해 정적 확장 목록에 추가. 데모는 lobby 카드 하나를 기여.

## 2. 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│ oxipage-server (feature "wasm")                             │
│   run_server_with_extensions:                               │
│     all = static_extensions()                               │
│     all.extend(oxipage_wasm::load_all_from_dir(             │
│                  data_dir/extensions))                      │
│   → ExtensionRegistry (DB state, gate, lobby manifest)      │
└──────────────────────────────┬──────────────────────────────┘
                               │ loads .wasm
┌──────────────────────────────▼──────────────────────────────┐
│ oxipage-wasm  (WasmExtensionAdapter: impl Extension)        │
│   wasmtime Engine/Module/Store/Linker                       │
│   build_linker(): import "env" host_now_unix / host_log     │
│   read_str_*arg(): memory.data() + ptr/len → String         │
└──────────────────────────────┬──────────────────────────────┘
                               │ core wasm ABI
┌──────────────────────────────▼──────────────────────────────┐
│ oxipage-ext-wasm-demo  (crate-type cdylib, no_std, wasm32)  │
│   export: memory, init, ext_id_*, display_name_*,           │
│           lobby_card_*  (lobby JSON = PREFIX + itoa(now))   │
│   import: env::host_now_unix, env::host_log                 │
└─────────────────────────────────────────────────────────────┘
```

설치 경로(어떤 빌드에서든 동작, wasmtime 불필요):

```
oxipage extension install wasm-demo
  → CLI: POST /api/v1/extensions/install {"name":"wasm-demo"}
  → core http::extension_install:
      registry/index.json (embedded) 조회 → runtime_loadable 확인
      bytes = embedded DEMO_WASM_BYTES  (또는 wasm_url http 다운로드)
      write data/extensions/wasm-demo.wasm
      INSERT extension_state (enabled=0)
  → "restart oxipage-server (--features wasm) to activate"
```

## 3. ABI 명세

### 모듈 export (코어 WASM)

| export | 시그니처 | 비고 |
|---|---|---|
| `memory` | linear memory | `--export-memory` 로 노출; 호스트가 ptr/len 으로 읽음 |
| `init` | `() -> ()` | load 직후 1회; capability 호출 + lobby 버퍼 구축 |
| `ext_id_ptr` / `ext_id_len` | `() -> i32` | UTF-8 id (예: `wasm-demo`) |
| `display_name_ptr` / `display_name_len` | `(i32 lang) -> i32` | `0`=ko, `1`=en |
| `lobby_card_ptr` / `lobby_card_len` | `() -> i32` | JSON `{"id","items":[{"title","url"}]}`; 빈 문자열 = 카드 없음 |

### 모듈 import (module = `"env"`)

| import | 시그니처 | capability |
|---|---|---|
| `host_now_unix` | `() -> i64` | read-only 시계 (epoch 초) |
| `host_log` | `(i32 ptr, i32 len) -> ()` | tracing 로깅 (게스트 → 호스트) |

문자열 전달 규약: 게스트가 linear memory 의 오프셋(ptr)과 길이(len) 을 i32 로
반환 → 호스트가 `memory.data(store)[ptr..ptr+len]` 을 UTF-8 로 읽는다. 게스트가
동적 데이터를 만들면 정적 버퍼에 쓴 뒤 그 버퍼의 ptr/len 을 반환한다 (데모의
`lobby_card_*`). 입력(호스트→게스트) 문자열 전달은 이 스파이크에선 필요 없다
(lobby 카드는 입력 없이 생성).

## 4. 알려진 한계 (doc/01 §1.4 와 일관)

1. **HTTP 라우트 없음.** `Extension::routes() -> Router<AppState>` 을 WASM 이
   생산할 수 없다 (`Router` 는 호스트 구체 타입). WASM 확장은 lobby 카드만 기여.
   → "런타임 설치 확장은 API/웹으로만" 의 근거가 된다.
2. **DB 마이그레이션 없음.** `migrations()`/`table_names()` 역시 호스트 타입.
   WASM 확장은 자체 테이블을 소유하지 않는다.
3. **CLI 서브커맨드 없음.** `clap` 이 정적 링크를 요구하므로 런타임 설치 확장이
   자기 서브커맨드를 추가할 수 없다 (`oxipage wasm-demo ...` 불가). install 자체는
   코어 CLI 명령(`oxipage extension install`)으로 동작.
4. **부팅 시에만 적재.** 라우트/레지스트리가 `build_app` 시점에 조립되므로, install
   후 활성화에는 서버 재기동이 필요하다 (스파이크 단순화; 핫 리로드는 future work).
5. **매 호출마다 store 인스턴스화.** `lobby_summary` 가 빈번하면 store 재사용으로
   최적화 가능하지만, 스파이크에선 단순화를 위해 매 호출 재인스턴스화.

## 5. 의도적 편차 (deliberate deviations)

- **코어 WASM 모듈 vs 컴포넌트 모델.** doc/01 §1.4 는 "WASM 컴포넌트 모델"을
  언급하지만, 본 스파이크는 `cargo-component`/`wit-bindgen` 없이 **코어 모듈 + 수동
  ABI**를 쓴다 (~10배 단순, 환경 의존성 제로). 컴포넌트 모델은 프로덕션 하드닝
  타겟(§7).
- **`Extension::id` 시그니처 축소.** `fn id(&self) -> &'static str` → `fn id(&self)
  -> &str`. 런타임 학습 id 를 `&'static` 으로 만들려면 leak 이나 unsafe 가 필요한데,
  시그니처 한 줄 줄이는 쪽이 원인 제거다. 모든 기존 구현(문자열 리터럴)은 그대로
  컴파일되고, 런타임 WASM id 는 `&self.id`(소유 String)를 빌려준다.
- **`wasm` cargo feature (기본 off).** wasmtime 링크/바이너리 크기를 피하려고
  서버 통합을 `--features wasm` 게이트로 뒀다. 기본 빌드/배포는 영향 없음.
  install 엔드포인트 자체는 모든 빌드에서 동작(바이트 저장 + DB 만 하므로).

## 6. 빌드 / 실행 / 검증

### 데모 `.wasm` 빌드

```sh
cargo build -p oxipage-ext-wasm-demo --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/oxipage_ext_wasm_demo.wasm \
   crates/oxipage-ext-wasm-demo/artifacts/wasm-demo.wasm
```

`.cargo/config.toml` 의 `[target.wasm32-unknown-unknown]` rustflags 가
`--export-memory` + 각 `--export=<sym>` 을 주입한다 (cargo 가 config 를 cwd 기준
상향 탐색하므로 워크스페이스 루트에 있다).

### 호스트 단위 테스트

```sh
cargo test -p oxipage-wasm   # loads artifact, asserts id/display/lobby
```

- `loads_demo_and_extracts_static_metadata`: memory + ext_id/display_name export 검증.
- `lobby_summary_returns_a_card`: 동적 lobby JSON (host_now_unix 결과 포함) round-trip.
- `missing_dir_is_empty_not_error`: 로더 복원력.

### 설치 round-trip

```sh
cargo test -p oxipage-core --test http_app install_writes_wasm
```

`POST /extensions/install {"name":"wasm-demo"}` → 200, 파일 쓰기, `extension_state`
행(enabled=0) 검증. 하이픈 이름이 `is_safe_extension_name` 을 통과하는지도 확인.

### wasm-feature 서버

```sh
cargo build -p oxipage-server --features wasm
# 실행 후 data/extensions/*.wasm 이 부팅 시 적재되어 lobby 매니페스트에 카드로 나타남.
```

### 품질 게이트

```sh
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace                                   # 108 tests ok (3 wasm host + 설치 round-trip 포함)
```

## 7. 다음 단계 (프로덕션 하드닝, 범위 밖)

1. **컴포넌트 모델(WIT/wasip2) 마이그레이션.** `cargo-component` + `wit-bindgen` 으로
   ABI 를 WIT 인터페이스로 승격. ptr/len 수동 직렬화 제거, 타입 안전한 바인딩.
2. **capability 확장.** `host_db_query(sql, params) -> json` (read-only, 화이트리스트),
   `host_http_get(url) -> bytes` (도메인 화이트리스트). capability 토큰으로 per-extension
   권한 부여.
3. **핫 리로드.** install 후 재기동 없이 registry 에 live 추가. 라우트는 여전히 불가하지만
   lobby 카드는 즉시 반영 가능.
4. **리소스 제한.** wasmtime `Store` 에 fuel/epoch 로 CPU, 메모리 상한으로 OOM 방지.
5. **서명 검증.** `.wasm` 아티팩트의 서명(cosign/minisign) 검증 후 install — 레지스트리
   신뢰 모델 확립.
6. **원격 레지스트리.** `registry/index.json` 을 GitHub 호스팅으로 옮기고 `wasm_url` 을
   릴리스 자산으로. 현재 데모는 임베드 바이트로 오프라인 검증.

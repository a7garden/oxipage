//! WASM 런타임 적재 호스트 (doc/01 §1.4, doc/08 §8.4).
//!
//! `Extension` trait은 `Router<AppState>`/`Vec<Arc<dyn ScheduledJob>>` 같은
//! 호스트 구체 타입을 반환하므로, 이들은 WASM 경계를 넘을 수 없다. 따라서 런타임
//! 적재 확장은 trait의 **서브셋 ABI**만 미러링한다:
//!
//! - `id()`, `display_name(lang)`, `lobby_summary()`, `on_startup()`
//!
//! 라우트·마이그레이션·백그라운드 잡은 WASM 확장이 제공하지 못한다 (알려진 한계,
//! doc/01 §1.4 "런타임 설치 확장은 API/웹으로만"의 근거). 호스트 어댑터가 이 ABI
//! 위에 완전한 `Extension`을 구현한다 — 빈 `routes()`/`migrations()`를 반환.
//!
//! ## 스파이크 범위 (deliberate deviation)
//!
//! doc/01 §1.4는 "WASM 컴포넌트 모델"을 언급하지만, 본 스파이크는 **코어 WASM
//! 모듈 + 수동 ABI**를 쓴다 (`cargo-component`/`wit-bindgen` 미설치 환경에서
//! 단순). 컴포넌트 모델(WIT/wasip2)은 프로덕션 하드닝 타겟으로 `docs/wasm-spike.md`에 남긴다.
//!
//! ## ABI (코어 모듈 export)
//!
//! | export | 시그니처 | 비고 |
//! |---|---|---|
//! | `memory` | linear memory | 호스트가 ptr/len 으로 읽는다 |
//! | `init` | `() -> ()` | 시작 1회; 호스트 capability 호출 |
//! | `ext_id_ptr` / `ext_id_len` | `() -> i32` | UTF-8 id |
//! | `display_name_ptr` / `display_name_len` | `(i32 lang) -> i32` | 0=ko,1=en |
//! | `lobby_card_ptr` / `lobby_card_len` | `() -> i32` | JSON or 빈 문자열=없음 |
//!
//! ## 호스트 capability (모듈 import, module="env")
//!
//! | import | 시그니처 | capability |
//! |---|---|---|
//! | `host_now_unix` | `() -> i64` | 현재 epoch 초 (read-only 시계) |
//! | `host_log` | `(i32 ptr, i32 len) -> ()` | tracing 로깅 |
//!
//! DB/HTTP capability는 동일한 링커 경로로 추가 가능하나, 본 스파이크는 capability
//! 메커니즘 자체를 증명하기 위해 위 두 개만 제공한다 (다음 단계는 docs 참조).

use async_trait::async_trait;
use axum::Router;
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::scheduler::ScheduledJob;
use oxipage_core::state::AppState;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

/// import module name. 데모 모듈은 `import "env" "host_*"` 로 선언한다.
const IMPORT_MODULE: &str = "env";

/// 로드 가능한 WASM 확장 하나. 엔진은 각 확장이 소유한다 (스파이크 단순화).
pub struct WasmExtensionAdapter {
    module: Module,
    /// 모듈에서 추출한 id (정적). trait 이 `id() -> &str` 이므로 그냥 빌려준다.
    id: String,
    display_ko: String,
    display_en: String,
}

impl WasmExtensionAdapter {
    /// `.wasm` 파일에서 확장을 로드. id/display_name 을 즉시 추출(정적 데이터).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let engine = Engine::default();
        let wasm = std::fs::read(path)?;
        let module = Module::new(&engine, &wasm)?;
        let mut store = Store::new(&engine, ());
        let instance = instantiate(&mut store, &module)?;

        // init() 호출 — capability 연결 증명 (host_log/now).
        call_init(&mut store, &instance)?;
        let id = read_str_0arg(&mut store, &instance, "ext_id_ptr", "ext_id_len")?;
        let display_ko = read_str_1arg(&mut store, &instance, "display_name_ptr", "display_name_len", 0)?;
        let display_en = read_str_1arg(&mut store, &instance, "display_name_ptr", "display_name_len", 1)?;

        Ok(Self {
            module,
            id,
            display_ko,
            display_en,
        })
    }

    /// 매 호출마다 새 store 인스턴스화(동적 lobby 데이터). 스파이크 단순화;
    /// lobby_summary 가 빈번하면 store 재사용으로 최적화 (TODO docs).
    fn lobby_card(&self) -> anyhow::Result<Option<LobbyCard>> {
        let engine = self.module.engine();
        let mut store = Store::new(engine, ());
        let instance = instantiate(&mut store, &self.module)?;
        call_init(&mut store, &instance)?;
        let json = read_str_0arg(&mut store, &instance, "lobby_card_ptr", "lobby_card_len")?;
        if json.is_empty() {
            return Ok(None);
        }
        let parsed: LobbyJson = serde_json::from_str(&json)?;
        Ok(Some(LobbyCard {
            id: parsed.id,
            items: parsed
                .items
                .into_iter()
                .map(|i| LobbyCardItem {
                    title: i.title,
                    url: i.url,
                })
                .collect(),
        }))
    }
}

#[async_trait]
impl Extension for WasmExtensionAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => self.display_ko.clone(),
            Lang::En => self.display_en.clone(),
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        // WASM 확장은 DB 테이블을 소유하지 않는다 (알려진 한계).
        Vec::new()
    }

    fn routes(&self) -> Router<AppState> {
        // WASM 확장은 HTTP 라우트를 마운트하지 못한다 — 코어 WASM ABI 는 라우트
        // 생성을 지원하지 않는다 (알려진 한계, doc/01 §1.4).
        Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        match self.lobby_card() {
            Ok(card) => card,
            Err(e) => {
                tracing::warn!(extension = %self.id, error = %e, "wasm lobby_summary failed");
                None
            }
        }
    }

    async fn on_startup(&self, _ctx: &AppState) -> anyhow::Result<()> {
        // init() 은 load 시점에 이미 1회 호출됨. 여기선 no-op.
        Ok(())
    }

    fn background_jobs(&self) -> Vec<Arc<dyn ScheduledJob>> {
        Vec::new()
    }
}

// ───────────────────────── 로더 ─────────────────────────

/// 디렉토리 내 `*.wasm` 을 모두 로드. 실패한 파일은 warn 후 스킵 (한 확장의
/// 손상이 전체 부팅을 막지 않게). 빈 디렉토리/없으면 빈 vec.
pub fn load_all_from_dir(dir: &Path) -> Vec<Arc<dyn Extension>> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        tracing::debug!(dir = %dir.display(), "wasm extensions dir missing — skipping");
        return out;
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "wasm"))
        .collect();
    paths.sort();
    for p in paths {
        match WasmExtensionAdapter::load(&p) {
            Ok(ext) => {
                tracing::info!(wasm = %p.display(), id = %ext.id, "loaded wasm extension");
                out.push(Arc::new(ext));
            }
            Err(e) => {
                tracing::warn!(wasm = %p.display(), error = %e, "failed to load wasm extension");
            }
        }
    }
    out
}

// ───────────────────────── ABI 헬퍼 ─────────────────────────

fn call_init(store: &mut Store<()>, instance: &Instance) -> anyhow::Result<()> {
    if let Ok(init) = instance.get_typed_func::<(), ()>(&mut *store, "init") {
        init.call(&mut *store, ())?;
    }
    Ok(())
}

fn instantiate(store: &mut Store<()>, module: &Module) -> anyhow::Result<Instance> {
    let linker = build_linker(module.engine());
    let instance = linker.instantiate(&mut *store, module)?;
    Ok(instance)
}

/// capability-based 호스트 함수 링커. 새 capability 추가 시 여기에 import 등록.
fn build_linker(engine: &Engine) -> Linker<()> {
    let mut linker = Linker::new(engine);

    // host_now_unix: read-only 시계. capability 의 증명용 최소 surface.
    let _ = linker.func_wrap(IMPORT_MODULE, "host_now_unix", || -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    });

    // host_log: ptr/len 으로 게스트 메모리의 UTF-8 을 읽어 tracing 출력.
    let _ = linker.func_wrap(
        IMPORT_MODULE,
        "host_log",
        |mut caller: Caller<'_, ()>, ptr: i32, len: i32| {
            if let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                let data = mem.data(&caller);
                if let Some(slice) = sub_slice(data, ptr, len)
                    && let Ok(msg) = std::str::from_utf8(slice)
                {
                    tracing::info!(target: "oxipage::wasm::guest", "{msg}");
                }
            }
        },
    );

    linker
}

fn read_str_0arg(
    store: &mut Store<()>,
    instance: &Instance,
    ptr_name: &str,
    len_name: &str,
) -> anyhow::Result<String> {
    let mem = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| anyhow::anyhow!("wasm module missing `memory` export"))?;
    let ptr = instance
        .get_typed_func::<(), i32>(&mut *store, ptr_name)?
        .call(&mut *store, ())?;
    let len = instance
        .get_typed_func::<(), i32>(&mut *store, len_name)?
        .call(&mut *store, ())?;
    let data = mem.data(&*store);
    let slice = sub_slice(data, ptr, len)
        .ok_or_else(|| anyhow::anyhow!("wasm string ptr/len out of bounds"))?;
    Ok(std::str::from_utf8(slice)?.to_owned())
}

fn read_str_1arg(
    store: &mut Store<()>,
    instance: &Instance,
    ptr_name: &str,
    len_name: &str,
    arg: i32,
) -> anyhow::Result<String> {
    let mem = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| anyhow::anyhow!("wasm module missing `memory` export"))?;
    let ptr = instance
        .get_typed_func::<i32, i32>(&mut *store, ptr_name)?
        .call(&mut *store, arg)?;
    let len = instance
        .get_typed_func::<i32, i32>(&mut *store, len_name)?
        .call(&mut *store, arg)?;
    let data = mem.data(&*store);
    let slice = sub_slice(data, ptr, len)
        .ok_or_else(|| anyhow::anyhow!("wasm string ptr/len out of bounds"))?;
    Ok(std::str::from_utf8(slice)?.to_owned())
}

fn sub_slice(data: &[u8], ptr: i32, len: i32) -> Option<&[u8]> {
    let start = usize::try_from(ptr.max(0)).ok()?;
    let end = usize::try_from(len.max(0)).ok()?.checked_add(start)?;
    data.get(start..end)
}

#[derive(serde::Deserialize)]
struct LobbyJson {
    id: String,
    items: Vec<LobbyItemJson>,
}

#[derive(serde::Deserialize)]
struct LobbyItemJson {
    title: String,
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO_WASM: &str = "../oxipage-ext-wasm-demo/artifacts/wasm-demo.wasm";

    fn demo_available() -> Option<&'static Path> {
        let p = Path::new(DEMO_WASM);
        p.exists().then_some(p)
    }

    #[test]
    fn loads_demo_and_extracts_static_metadata() {
        let Some(path) = demo_available() else {
            eprintln!(
                "skipped: {DEMO_WASM} not built — run \
                 `cargo build -p oxipage-ext-wasm-demo --target wasm32-unknown-unknown --release`"
            );
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        assert_eq!(ext.id, "wasm-demo");
        assert_eq!(ext.display_ko, "WASM 데모");
        assert_eq!(ext.display_en, "WASM Demo");
    }

    #[test]
    fn lobby_summary_returns_a_card() {
        let Some(path) = demo_available() else {
            eprintln!("skipped: {DEMO_WASM} not built");
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        let card = ext.lobby_card().expect("lobby_card call").expect("some card");
        assert_eq!(card.id, "wasm-demo");
        assert!(card.items.iter().any(|i| i.title.contains("WASM")), "card: {card:?}");
    }

    #[test]
    fn missing_dir_is_empty_not_error() {
        let v = load_all_from_dir(Path::new("/nonexistent/oxipage-wasm-test"));
        assert!(v.is_empty());
    }
}

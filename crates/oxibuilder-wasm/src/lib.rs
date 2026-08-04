//! WASM 런타임 적재 호스트 v2 (doc/01 §1.4, doc/08 §8.4).
//!
//! v1 스파이크의 한계 3종을 해결한다:
//!
//! - **Store 재사용**: lobby Store+Instance 를 `Mutex` 로 캐싱하여 매 호출 재인스턴스화 제거.
//! - **Fuel 제한**: `Config::consume_fuel(true)` + `Store::set_fuel(N)` 으로 CPU 과다 방지.
//! - **HTTP 라우트**: route manifest ABI + `handle_request` 로 WASM 확장이 라우트를 제공.
//!   코어 `RouteDispatcher` trait 구현 → 폴백 핸들러가 디스패치.
//!
//! 추가 capability (doc/08 §8.4 §7):
//! - `host_db_query(sql) -> json` — read-only SQL (SELECT 만 허용).
//! - `host_http_get(url) -> body` — HTTP GET (도메인 제한 없음, 데모용).
//!
//! ## ABI (코어 모듈 export)
//!
//! | export | 시그니처 | 비고 |
//! |---|---|---|
//! | `memory` | linear memory | `--export-memory` |
//! | `init` | `() -> ()` | 시작 1회; capability 호출 |
//! | `ext_id_ptr` / `ext_id_len` | `() -> i32` | UTF-8 id |
//! | `display_name_ptr` / `display_name_len` | `(i32 lang) -> i32` | 0=ko, 1=en |
//! | `lobby_card_ptr` / `lobby_card_len` | `() -> i32` | JSON or 빈 문자열 |
//! | `alloc` | `(i32 size) -> i32 ptr` | linear memory 할당 (bump) |
//! | `reset_alloc` | `() -> ()` | 할당자 리셋 (요청 전) |
//! | `route_manifest_ptr` / `route_manifest_len` | `() -> i32` | JSON 라우트 목록 |
//! | `handle_request` | `(i32 method, i32 path_ptr, i32 path_len, i32 body_ptr, i32 body_len) -> i64` | `(status << 32) \| resp_ptr` |
//! | `handle_request_len` | `() -> i32` | 응답 본문 길이 |
//!
//! ## 호스트 capability (모듈 import, module="env")
//!
//! | import | 시그니처 | capability |
//! |---|---|---|
//! | `host_now_unix` | `() -> i64` | 현재 epoch 초 (read-only 시계) |
//! | `host_log` | `(i32 ptr, i32 len) -> ()` | tracing 로깅 |
//! | `host_db_query` | `(i32 sql_ptr, i32 sql_len) -> i64` | read-only SQL → JSON |
//! | `host_http_get` | `(i32 url_ptr, i32 url_len) -> i64` | HTTP GET → body |

use async_trait::async_trait;
use axum::Router;
use oxibuilder_core::extension::{
    Extension, Lang, LobbyCard, LobbyCardItem, Migration, RouteDispatcher, RouteResponse,
    RouteSpec, WasmLoader,
};
use oxibuilder_core::scheduler::ScheduledJob;
use oxibuilder_core::state::AppState;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use wasmtime::{Caller, Config, Engine, Instance, Linker, Module, Store};

const IMPORT_MODULE: &str = "env";
/// 라우트 요청당 fuel (약 10M unit ≈ 수십 ms CPU).
const FUEL_PER_REQUEST: u64 = 10_000_000;
/// lobby 카드 생성당 fuel.
const FUEL_FOR_LOBBY: u64 = 1_000_000;
/// host_db_query / host_http_get 결과 버퍼 최대 크기.
const MAX_CAPABILITY_RESPONSE: usize = 256 * 1024;

/// 호스트 capability 함수에 전달되는 store 상태.
struct WasmHostState {
    /// DB pool. load 시점(메타데이터 추출)에는 None.
    db: Option<sqlx::SqlitePool>,
}

/// WASM 런타임 로더 (서버가 `--features wasm` 일 때 주입).
pub struct WasmLoaderImpl;

impl WasmLoader for WasmLoaderImpl {
    fn load(&self, path: &Path) -> anyhow::Result<Arc<dyn Extension>> {
        Ok(Arc::new(WasmExtensionAdapter::load(path)?))
    }
}

/// 로드 가능한 WASM 확장 하나. Engine+Module 은 공유, lobby Store 는 재사용(Mutex 캐싱).
pub struct WasmExtensionAdapter {
    engine: Engine,
    module: Module,
    id: String,
    display_ko: String,
    display_en: String,
    route_specs: Vec<RouteSpec>,
    /// lobby 카드용 영구 Store+Instance. Mutex 로 보호, 매 호출 재사용.
    lobby: Mutex<LobbyCache>,
}

struct LobbyCache {
    store: Store<WasmHostState>,
    instance: Instance,
}

impl WasmExtensionAdapter {
    /// `.wasm` 파일에서 확장을 로드. id/display_name/route_manifest 를 즉시 추출.
    /// lobby 용 Store+Instance 를 생성하고 init() 을 1회 호출해 캐싱한다.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let engine = make_engine();
        let wasm = std::fs::read(path)?;
        let module = Module::new(&engine, &wasm)?;

        // 메타데이터 추출용 1회성 store.
        let mut store = Store::new(&engine, WasmHostState { db: None });
        let _ = store.set_fuel(FUEL_FOR_LOBBY);
        let instance = instantiate(&mut store, &module)?;
        call_init(&mut store, &instance)?;

        let id = read_str_0arg(&mut store, &instance, "ext_id_ptr", "ext_id_len")?;
        let display_ko = read_str_1arg(
            &mut store,
            &instance,
            "display_name_ptr",
            "display_name_len",
            0,
        )?;
        let display_en = read_str_1arg(
            &mut store,
            &instance,
            "display_name_ptr",
            "display_name_len",
            1,
        )?;

        // route manifest 추출 (optional — v1 모듈은 export 하지 않음).
        let route_specs = read_route_manifest(&mut store, &instance).unwrap_or_default();

        // lobby 캐시용 영구 store 생성 + init() 1회.
        let mut lobby_store = Store::new(&engine, WasmHostState { db: None });
        let _ = lobby_store.set_fuel(FUEL_FOR_LOBBY);
        let lobby_instance = instantiate(&mut lobby_store, &module)?;
        call_init(&mut lobby_store, &lobby_instance)?;

        Ok(Self {
            engine,
            module,
            id,
            display_ko,
            display_en,
            route_specs,
            lobby: Mutex::new(LobbyCache {
                store: lobby_store,
                instance: lobby_instance,
            }),
        })
    }

    /// lobby 카드 읽기 (캐싱된 store 재사용).
    fn lobby_card(&self) -> anyhow::Result<Option<LobbyCard>> {
        let mut cache = self.lobby.lock().unwrap();
        // fuel 보충 — 이전 호출로 소진되었을 수 있음.
        let cache = &mut *cache;
        let (store, instance) = (&mut cache.store, &cache.instance);
        let _ = store.set_fuel(FUEL_FOR_LOBBY);
        let json = read_str_0arg(store, instance, "lobby_card_ptr", "lobby_card_len")?;
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

    /// 라우트 요청 디스패치 (fresh store — fuel isolation per request).
    fn dispatch_request(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        db: Option<sqlx::SqlitePool>,
    ) -> RouteResponse {
        let result = (|| -> anyhow::Result<RouteResponse> {
            let mut store = Store::new(&self.engine, WasmHostState { db });
            let _ = store.set_fuel(FUEL_PER_REQUEST);
            let instance = instantiate(&mut store, &self.module)?;

            // alloc 이 없으면 디스패치 불가 (v1 모듈).
            let alloc = instance.get_typed_func::<i32, i32>(&mut store, "alloc")?;

            // reset_alloc (있으면 호출).
            if let Ok(reset) = instance.get_typed_func::<(), ()>(&mut store, "reset_alloc") {
                reset.call(&mut store, ())?;
            }

            // path 를 WASM 메모리에 쓰기.
            let path_bytes = path.as_bytes();
            let path_ptr = alloc.call(&mut store, path_bytes.len() as i32)?;
            if path_ptr == 0 {
                anyhow::bail!("wasm alloc returned 0 for path");
            }
            write_to_memory(&mut store, &instance, path_ptr, path_bytes)?;

            // body 를 WASM 메모리에 쓰기.
            let body_ptr = if body.is_empty() {
                0
            } else {
                let ptr = alloc.call(&mut store, body.len() as i32)?;
                if ptr == 0 {
                    anyhow::bail!("wasm alloc returned 0 for body");
                }
                write_to_memory(&mut store, &instance, ptr, body)?;
                ptr
            };

            // handle_request 호출.
            let method_code = method_to_code(method);
            let handle = instance
                .get_typed_func::<(i32, i32, i32, i32, i32), i64>(&mut store, "handle_request")?;
            let packed = handle.call(
                &mut store,
                (
                    method_code,
                    path_ptr,
                    path_bytes.len() as i32,
                    body_ptr,
                    body.len() as i32,
                ),
            )?;

            let status = ((packed >> 32) & 0xFFFF) as u16;
            let resp_ptr = (packed & 0xFFFFFFFF) as i32;

            // 응답 본문 읽기.
            let resp_len = if resp_ptr == 0 {
                0
            } else {
                instance
                    .get_typed_func::<(), i32>(&mut store, "handle_request_len")?
                    .call(&mut store, ())?
            };

            let resp_body = if resp_ptr != 0 && resp_len > 0 {
                let mem = instance
                    .get_memory(&mut store, "memory")
                    .ok_or_else(|| anyhow::anyhow!("missing memory export"))?;
                let data = mem.data(&store);
                sub_slice(data, resp_ptr, resp_len)
                    .ok_or_else(|| anyhow::anyhow!("response ptr/len out of bounds"))?
                    .to_vec()
            } else {
                Vec::new()
            };

            Ok(RouteResponse {
                status: if status == 0 { 200 } else { status },
                body: resp_body,
            })
        })();

        match result {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!(extension = %self.id, error = %e, "wasm dispatch failed");
                RouteResponse {
                    status: 500,
                    body: br#"{"error":"wasm_dispatch_failed"}"#.to_vec(),
                }
            }
        }
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
        Vec::new()
    }

    fn routes(&self) -> axum::Router {
        // WASM 확장은 폴백 핸들러가 동적 디스패치하므로 빈 Router 반환.
        Router::new()
    }

    fn route_dispatcher(&self) -> Option<&dyn RouteDispatcher> {
        if self.route_specs.is_empty() {
            None
        } else {
            Some(self)
        }
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
        Ok(())
    }

    fn background_jobs(&self) -> Vec<Arc<dyn ScheduledJob>> {
        Vec::new()
    }
}

#[async_trait]
impl RouteDispatcher for WasmExtensionAdapter {
    fn route_specs(&self) -> &[RouteSpec] {
        &self.route_specs
    }

    async fn dispatch(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
        ctx: &AppState,
    ) -> RouteResponse {
        self.dispatch_request(method, path, &body, Some(ctx.db.clone()))
    }
}

// ───────────────────────── 로더 ─────────────────────────

/// 디렉토리 내 `*.wasm` 을 모두 로드. 실패한 파일은 warn 후 스킵.
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

fn make_engine() -> Engine {
    let mut config = Config::new();
    config.consume_fuel(true);
    Engine::new(&config).expect("wasmtime engine creation should not fail")
}

fn call_init(store: &mut Store<WasmHostState>, instance: &Instance) -> anyhow::Result<()> {
    if let Ok(init) = instance.get_typed_func::<(), ()>(&mut *store, "init") {
        init.call(&mut *store, ())?;
    }
    Ok(())
}

fn instantiate(store: &mut Store<WasmHostState>, module: &Module) -> anyhow::Result<Instance> {
    let linker = build_linker(module.engine());
    let instance = linker.instantiate(&mut *store, module)?;
    Ok(instance)
}

/// capability-based 호스트 함수 링커. 새 capability 추가 시 여기에 import 등록.
fn build_linker(engine: &Engine) -> Linker<WasmHostState> {
    let mut linker = Linker::new(engine);

    // host_now_unix: read-only 시계.
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
        |mut caller: Caller<'_, WasmHostState>, ptr: i32, len: i32| {
            if let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                let data = mem.data(&caller);
                if let Some(slice) = sub_slice(data, ptr, len)
                    && let Ok(msg) = std::str::from_utf8(slice)
                {
                    tracing::info!(target: "oxibuilder::wasm::guest", "{msg}");
                }
            }
        },
    );

    // host_db_query: read-only SQL 실행 → JSON 결과를 모듈 메모리에 alloc 해서 반환.
    // 반환값: (result_ptr as i64) | ((result_len as i64) << 32). 0 = 에러.
    let _ = linker.func_wrap(
        IMPORT_MODULE,
        "host_db_query",
        |mut caller: Caller<'_, WasmHostState>, sql_ptr: i32, sql_len: i32| -> i64 {
            host_db_query_impl(&mut caller, sql_ptr, sql_len)
        },
    );

    // host_http_get: URL GET → body 를 모듈 메모리에 alloc 해서 반환.
    let _ = linker.func_wrap(
        IMPORT_MODULE,
        "host_http_get",
        |mut caller: Caller<'_, WasmHostState>, url_ptr: i32, url_len: i32| -> i64 {
            host_http_get_impl(&mut caller, url_ptr, url_len)
        },
    );

    linker
}

/// host_db_query 구현. SELECT 만 허용. 결과를 JSON 으로 직렬화해 모듈의 alloc 으로
/// 획득한 버퍼에 쓰고 packed ptr+len 반환.
fn host_db_query_impl(caller: &mut Caller<'_, WasmHostState>, sql_ptr: i32, sql_len: i32) -> i64 {
    // 1. SQL 문자열 읽기.
    let sql = match read_guest_string(caller, sql_ptr, sql_len) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    // 2. read-only 검증 (SELECT 로 시작).
    let trimmed = sql.trim();
    if !trimmed.to_ascii_uppercase().starts_with("SELECT") {
        tracing::warn!(target: "oxibuilder::wasm::capability", "host_db_query rejected: not SELECT");
        return 0;
    }

    // 3. DB pool 확인.
    let Some(db) = &caller.data().db.clone() else {
        tracing::warn!(target: "oxibuilder::wasm::capability", "host_db_query: no db in store");
        return 0;
    };

    // 4. 동기 실행 (multi-thread runtime 전용 — block_in_place).
    let json_result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let rows = sqlx::query(trimmed).fetch_all(db).await;
            rows.map(|rows| rows_to_json(&rows))
        })
    });

    let json = match json_result {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(target: "oxibuilder::wasm::capability", error = %e, "host_db_query failed");
            return 0;
        }
    };

    // 5. 결과를 모듈 메모리에 alloc + write.
    write_capability_result(caller, json.as_bytes())
}

/// host_http_get 구현. URL GET → body bytes.
fn host_http_get_impl(caller: &mut Caller<'_, WasmHostState>, url_ptr: i32, url_len: i32) -> i64 {
    let url = match read_guest_string(caller, url_ptr, url_len) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let body_result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status()));
            }
            resp.bytes()
                .await
                .map_err(|e| e.to_string())
                .map(|b| b.to_vec())
        })
    });

    let body = match body_result {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(target: "oxibuilder::wasm::capability", error = %e, "host_http_get failed");
            return 0;
        }
    };

    // 응답이 너무 크면 잘라냄.
    let body = if body.len() > MAX_CAPABILITY_RESPONSE {
        tracing::warn!(target: "oxibuilder::wasm::capability", "host_http_get response truncated");
        &body[..MAX_CAPABILITY_RESPONSE]
    } else {
        &body
    };

    write_capability_result(caller, body)
}

/// 결과를 모듈의 alloc 으로 획득한 버퍼에 쓰고 packed (ptr | len << 32) 반환.
fn write_capability_result(caller: &mut Caller<'_, WasmHostState>, data: &[u8]) -> i64 {
    // alloc 함수 획득 + 호출 (untyped — typed API 가 borrow 로 까다로움).
    let Some(alloc_func) = caller.get_export("alloc").and_then(|e| e.into_func()) else {
        tracing::warn!(target: "oxibuilder::wasm::capability", "module missing alloc export");
        return 0;
    };
    let params = [wasmtime::Val::I32(data.len() as i32)];
    let mut results = [wasmtime::Val::I32(0)];
    if alloc_func
        .call(&mut *caller, &params, &mut results)
        .is_err()
    {
        return 0;
    };
    let ptr = results[0].i32().unwrap_or(0);
    if ptr == 0 {
        return 0;
    }

    // 메모리에 결과 쓰기.
    let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return 0;
    };
    let mem_data = mem.data_mut(&mut *caller);
    let Some(slice) = sub_slice_mut(mem_data, ptr, data.len() as i32) else {
        return 0;
    };
    slice.copy_from_slice(data);

    pack_ptr_len(ptr, data.len() as i32)
}

fn read_guest_string(
    caller: &mut Caller<'_, WasmHostState>,
    ptr: i32,
    len: i32,
) -> anyhow::Result<String> {
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("missing memory export"))?;
    let data = mem.data(caller);
    let slice =
        sub_slice(data, ptr, len).ok_or_else(|| anyhow::anyhow!("string ptr/len out of bounds"))?;
    Ok(std::str::from_utf8(slice)?.to_owned())
}

/// sqlx AnyRow → JSON 배열 직렬화. 컬럼명은 알 수 없으므로 인덱스 기반.
fn rows_to_json(rows: &[sqlx::sqlite::SqliteRow]) -> String {
    use sqlx::{Column, Row};
    let mut arr = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = serde_json::Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let name = col.name();
            let val = row
                .try_get::<Option<i64>, _>(i)
                .ok()
                .flatten()
                .map(serde_json::Value::from)
                .or_else(|| {
                    row.try_get::<Option<f64>, _>(i)
                        .ok()
                        .flatten()
                        .and_then(|v| {
                            serde_json::Number::from_f64(v).map(serde_json::Value::Number)
                        })
                })
                .or_else(|| {
                    row.try_get::<Option<String>, _>(i)
                        .ok()
                        .flatten()
                        .map(serde_json::Value::from)
                })
                .unwrap_or(serde_json::Value::Null);
            obj.insert(name.to_string(), val);
        }
        arr.push(serde_json::Value::Object(obj));
    }
    serde_json::Value::Array(arr).to_string()
}

fn write_to_memory(
    store: &mut Store<WasmHostState>,
    instance: &Instance,
    ptr: i32,
    data: &[u8],
) -> anyhow::Result<()> {
    let mem = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| anyhow::anyhow!("missing memory export"))?;
    let mem_data = mem.data_mut(&mut *store);
    let slice = sub_slice_mut(mem_data, ptr, data.len() as i32)
        .ok_or_else(|| anyhow::anyhow!("write ptr/len out of bounds"))?;
    slice.copy_from_slice(data);
    Ok(())
}

fn method_to_code(method: &str) -> i32 {
    match method {
        "GET" => 0,
        "POST" => 1,
        "PUT" => 2,
        "DELETE" => 3,
        "PATCH" => 4,
        _ => 0,
    }
}

fn read_str_0arg(
    store: &mut Store<WasmHostState>,
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
    store: &mut Store<WasmHostState>,
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

fn read_route_manifest(
    store: &mut Store<WasmHostState>,
    instance: &Instance,
) -> anyhow::Result<Vec<RouteSpec>> {
    let json = read_str_0arg(store, instance, "route_manifest_ptr", "route_manifest_len")?;
    if json.is_empty() {
        return Ok(Vec::new());
    }
    let entries: Vec<RouteManifestEntry> = serde_json::from_str(&json)?;
    Ok(entries
        .into_iter()
        .map(|e| RouteSpec {
            method: e.method,
            path: e.path,
        })
        .collect())
}

fn sub_slice(data: &[u8], ptr: i32, len: i32) -> Option<&[u8]> {
    let start = usize::try_from(ptr.max(0)).ok()?;
    let end = usize::try_from(len.max(0)).ok()?.checked_add(start)?;
    data.get(start..end)
}

fn sub_slice_mut(data: &mut [u8], ptr: i32, len: i32) -> Option<&mut [u8]> {
    let start = usize::try_from(ptr.max(0)).ok()?;
    let end = usize::try_from(len.max(0)).ok()?.checked_add(start)?;
    data.get_mut(start..end)
}

fn pack_ptr_len(ptr: i32, len: i32) -> i64 {
    (ptr as u32 as i64) | ((len as u32 as i64) << 32)
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

#[derive(serde::Deserialize)]
struct RouteManifestEntry {
    method: String,
    path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO_WASM: &str = "../oxibuilder-ext-wasm-demo/artifacts/wasm-demo.wasm";

    fn demo_available() -> Option<&'static Path> {
        let p = Path::new(DEMO_WASM);
        p.exists().then_some(p)
    }

    #[test]
    fn loads_demo_and_extracts_static_metadata() {
        let Some(path) = demo_available() else {
            eprintln!(
                "skipped: {DEMO_WASM} not built — run \
                 `cargo build -p oxibuilder-ext-wasm-demo --target wasm32-unknown-unknown --release`"
            );
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        assert_eq!(ext.id, "wasm-demo");
        assert_eq!(ext.display_ko, "WASM 데모 v2");
        assert_eq!(ext.display_en, "WASM Demo v2");
    }

    #[test]
    fn lobby_summary_returns_a_card() {
        let Some(path) = demo_available() else {
            eprintln!("skipped: {DEMO_WASM} not built");
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        let card = ext
            .lobby_card()
            .expect("lobby_card call")
            .expect("some card");
        assert_eq!(card.id, "wasm-demo");
        assert!(
            card.items.iter().any(|i| i.title.contains("WASM")),
            "card: {card:?}"
        );
    }

    #[test]
    fn route_manifest_extracted() {
        let Some(path) = demo_available() else {
            eprintln!("skipped: {DEMO_WASM} not built");
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        assert!(
            !ext.route_specs.is_empty(),
            "should have routes: {:?}",
            ext.route_specs
        );
        assert!(
            ext.route_specs.iter().any(|r| r.path.contains("info")),
            "should have /info route: {:?}",
            ext.route_specs
        );
    }

    #[test]
    fn dispatch_returns_response() {
        let Some(path) = demo_available() else {
            eprintln!("skipped: {DEMO_WASM} not built");
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        let resp = ext.dispatch_request("GET", "/info", &[], None);
        assert_eq!(resp.status, 200, "status: {}", resp.status);
        let body = std::str::from_utf8(&resp.body).unwrap_or("");
        assert!(body.contains("wasm-demo"), "body: {body}");
    }

    #[test]
    fn missing_dir_is_empty_not_error() {
        let v = load_all_from_dir(Path::new("/nonexistent/oxibuilder-wasm-test"));
        assert!(v.is_empty());
    }

    #[test]
    fn store_reuse_for_lobby() {
        let Some(path) = demo_available() else {
            eprintln!("skipped: {DEMO_WASM} not built");
            return;
        };
        let ext = WasmExtensionAdapter::load(path).expect("load demo wasm");
        // 두 번째 호출이 캐싱된 store 를 재사입하는지 확인 (패닉 없이 통과).
        let _c1 = ext.lobby_card().expect("first lobby_card");
        let _c2 = ext.lobby_card().expect("second lobby_card (reused store)");
    }
}

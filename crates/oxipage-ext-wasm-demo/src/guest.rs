//! 코어 WASM 모듈 구현 v2 (target_arch = "wasm32").
//!
//! 호스트 capability import + ABI export. 스택/데이터 세그먼트만 사용 (힙 없음).
//! v2 추가: alloc/reset_alloc, route_manifest, handle_request,
//! host_db_query / host_http_get capability import.
//!
//! Rust 2024: static_mut_refs 를 회피하기 위해 모든 static mut 접근은
//! addr_of!/addr_of_mut! + raw pointer 로만 수행한다.

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn host_now_unix() -> i64;
    fn host_log(ptr: i32, len: i32);
    fn host_db_query(sql_ptr: i32, sql_len: i32) -> i64;
    fn host_http_get(url_ptr: i32, url_len: i32) -> i64;
}

// ─── 정적 데이터 ───
const ID: &[u8] = b"wasm-demo";
const NAME_KO: &[u8] = "WASM 데모 v2".as_bytes();
const NAME_EN: &[u8] = b"WASM Demo v2";
const LOG_MSG: &[u8] = b"wasm-demo v2: init ok";

const PREFIX: &[u8] = b"{\"id\":\"wasm-demo\",\"items\":[{\"title\":\"WASM Demo v2 loaded@";
const SUFFIX: &[u8] = b"\",\"url\":\"https://oxipage.dev\"}]}";

const ROUTE_MANIFEST: &[u8] =
    b"[{\"method\":\"GET\",\"path\":\"/info\"},{\"method\":\"GET\",\"path\":\"/time\"},{\"method\":\"GET\",\"path\":\"/db\"}]";

const INFO_JSON: &[u8] = b"{\"extension\":\"wasm-demo\",\"version\":\"v2\",\"routes\":[\"/info\",\"/time\",\"/db\"]}";
const DB_ERROR_JSON: &[u8] = b"{\"error\":\"db_query_failed\"}";
const NOT_FOUND_JSON: &[u8] = b"{\"error\":\"not_found\"}";

// ─── 동적 버퍼 (static mut, raw pointer 로만 접근) ───
static mut LOBBY_BUF: [u8; 256] = [0; 256];
static mut LOBBY_LEN: i32 = 0;
static mut HEAP: [u8; 16384] = [0; 16384];
static mut HEAP_OFFSET: usize = 0;
const HEAP_SIZE: usize = 16384;
static mut RESP_BUF: [u8; 8192] = [0; 8192];
static mut RESP_LEN: i32 = 0;
static mut RESPONSE_STATUS: u16 = 200;
const RESP_SIZE: usize = 8192;

// ─── alloc / reset_alloc ───

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: i32) -> i32 {
    unsafe {
        let size = size as usize;
        if HEAP_OFFSET.checked_add(size).map_or(true, |end| end > HEAP_SIZE) {
            return 0;
        }
        let base = core::ptr::addr_of!(HEAP) as *const u8;
        let ptr = base.add(HEAP_OFFSET) as i32;
        HEAP_OFFSET += size;
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_alloc() {
    unsafe {
        HEAP_OFFSET = 0;
    }
}

// ─── init ───

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    unsafe {
        let now = host_now_unix();
        let base = core::ptr::addr_of_mut!(LOBBY_BUF) as *mut u8;
        let mut off: usize = 0;
        off += write_bytes(base, off, PREFIX);
        off += write_u64(base, off, now as u64);
        off += write_bytes(base, off, SUFFIX);
        LOBBY_LEN = off as i32;
        host_log(LOG_MSG.as_ptr() as i32, LOG_MSG.len() as i32);
    }
}

fn write_bytes(base: *mut u8, off: usize, src: &[u8]) -> usize {
    unsafe {
        let mut i = 0;
        while i < src.len() {
            base.add(off + i).write(src[i]);
            i += 1;
        }
    }
    src.len()
}

fn write_u64(base: *mut u8, off: usize, n: u64) -> usize {
    unsafe {
        if n == 0 {
            base.add(off).write(b'0');
            return 1;
        }
        let mut tmp = [0u8; 20];
        let mut n = n;
        let mut i = 0;
        while n > 0 {
            tmp[i] = b'0' + (n % 10) as u8;
            i += 1;
            n /= 10;
        }
        let mut j = 0;
        while j < i {
            base.add(off + j).write(tmp[i - 1 - j]);
            j += 1;
        }
        i
    }
}

// ─── ABI export: id / display_name ───

#[unsafe(no_mangle)]
pub extern "C" fn ext_id_ptr() -> i32 {
    ID.as_ptr() as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn ext_id_len() -> i32 {
    ID.len() as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn display_name_ptr(lang: i32) -> i32 {
    pick(lang).as_ptr() as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn display_name_len(lang: i32) -> i32 {
    pick(lang).len() as i32
}

const fn pick(lang: i32) -> &'static [u8] {
    if lang == 0 {
        NAME_KO
    } else {
        NAME_EN
    }
}

// ─── ABI export: lobby 카드 ───

#[unsafe(no_mangle)]
pub extern "C" fn lobby_card_ptr() -> i32 {
    core::ptr::addr_of!(LOBBY_BUF) as *const u8 as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn lobby_card_len() -> i32 {
    unsafe { core::ptr::addr_of!(LOBBY_LEN).read_volatile() }
}

// ─── ABI export: route manifest ───

#[unsafe(no_mangle)]
pub extern "C" fn route_manifest_ptr() -> i32 {
    ROUTE_MANIFEST.as_ptr() as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn route_manifest_len() -> i32 {
    ROUTE_MANIFEST.len() as i32
}

// ─── ABI export: handle_request ───

#[unsafe(no_mangle)]
pub extern "C" fn handle_request(
    method: i32,
    path_ptr: i32,
    path_len: i32,
    _body_ptr: i32,
    _body_len: i32,
) -> i64 {
    if method != 0 {
        set_response(405, NOT_FOUND_JSON);
        return pack_response();
    }
    if heap_matches(path_ptr, path_len, b"/info") {
        set_response(200, INFO_JSON);
    } else if heap_matches(path_ptr, path_len, b"/time") {
        handle_time();
    } else if heap_matches(path_ptr, path_len, b"/db") {
        handle_db();
    } else {
        set_response(404, NOT_FOUND_JSON);
    }
    pack_response()
}

#[unsafe(no_mangle)]
pub extern "C" fn handle_request_len() -> i32 {
    unsafe { core::ptr::addr_of!(RESP_LEN).read_volatile() }
}

// ─── 라우트 핸들러 ───

fn handle_time() {
    unsafe {
        let now = host_now_unix();
        let base = core::ptr::addr_of_mut!(RESP_BUF) as *mut u8;
        let mut off = 0;
        off += write_bytes(base, off, b"{\"now\":");
        off += write_u64(base, off, now as u64);
        off += write_bytes(base, off, b"}");
        RESP_LEN = off as i32;
        RESPONSE_STATUS = 200;
    }
}

fn handle_db() {
    let sql = b"SELECT count(*) as cnt FROM sqlite_master";
    let packed = unsafe { host_db_query(sql.as_ptr() as i32, sql.len() as i32) };
    if packed == 0 {
        set_response(500, DB_ERROR_JSON);
        return;
    }
    let ptr = (packed & 0xFFFFFFFF) as i32;
    let len = ((packed >> 32) as u32) as usize;
    let copy_len = len.min(RESP_SIZE);
    // ptr is an absolute linear memory address (host wrote via alloc).
    unsafe {
        let src = ptr as *const u8;
        let dst = core::ptr::addr_of_mut!(RESP_BUF) as *mut u8;
        core::ptr::copy_nonoverlapping(src, dst, copy_len);
        RESP_LEN = copy_len as i32;
        RESPONSE_STATUS = 200;
    }
}

// ─── 헬퍼 ───

fn set_response(status: u16, body: &[u8]) {
    unsafe {
        let copy_len = body.len().min(RESP_SIZE);
        let dst = core::ptr::addr_of_mut!(RESP_BUF) as *mut u8;
        core::ptr::copy_nonoverlapping(body.as_ptr(), dst, copy_len);
        RESP_LEN = copy_len as i32;
        RESPONSE_STATUS = status;
    }
}

fn pack_response() -> i64 {
    let ptr = core::ptr::addr_of!(RESP_BUF) as *const u8 as i32 as u64;
    let st = unsafe { core::ptr::addr_of!(RESPONSE_STATUS).read_volatile() } as u64;
    ((st << 32) | ptr) as i64
}

/// HEAP[path_ptr..path_ptr+path_len] 이 needle 과 접두사 일치하는지.
/// raw pointer 바이트 단위 비교 (static mut 참조 생성 없음).
fn heap_matches(ptr: i32, len: i32, needle: &[u8]) -> bool {
    let nlen = needle.len();
    if len < 0 || (len as usize) < nlen {
        return false;
    }
    // ptr is an absolute linear memory address (from alloc). Read bytes directly.
    let start = ptr as *const u8;
    for (i, &b) in needle.iter().enumerate() {
        if unsafe { start.add(i).read() } != b {
            return false;
        }
    }
    true
}

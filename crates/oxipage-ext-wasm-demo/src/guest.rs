//! 코어 WASM 모듈 구현 (target_arch = "wasm32").
//!
//! 호스트 capability import + ABI export. 스택/데이터 세그먼트만 사용 (힙 없음).
//! 호스트가 `init()` → `lobby_card_ptr/len` 순으로 호출해 JSON 카드를 얻는다.

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    // 트랩 — 호스트가 호출 실패로 처리한다.
    loop {}
}

// ─── 호스트 capability import (module="env") ───
// edition 2024: extern block 은 unsafe 로 선언.
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn host_now_unix() -> i64;
    fn host_log(ptr: i32, len: i32);
}

// ─── 정적 데이터 (데이터 세그먼트 → linear memory) ───
const ID: &[u8] = b"wasm-demo";
const NAME_KO: &[u8] = "WASM 데모".as_bytes();
const NAME_EN: &[u8] = b"WASM Demo";
const LOG_MSG: &[u8] = b"wasm-demo: init ok";

// lobby 카드 JSON. 동적 부분은 host_now_unix() 결과 한 곳 (capability 호출 증명).
const PREFIX: &[u8] = b"{\"id\":\"wasm-demo\",\"items\":[{\"title\":\"WASM Demo loaded@";
const SUFFIX: &[u8] = b"\",\"url\":\"https://oxipage.dev\"}]}";

// 동적 lobby JSON 버퍼 (init 시 구축). raw pointer 로만 접근 (static_mut_refs 회피).
static mut LOBBY_BUF: [u8; 256] = [0; 256];
static mut LOBBY_LEN: i32 = 0;

/// 호스트가 load 직후 호출. host capability 를 호출하고 lobby JSON 을 구축한다.
#[unsafe(no_mangle)]
pub extern "C" fn init() {
    unsafe {
        let now = host_now_unix();
        let base = core::ptr::addr_of_mut!(LOBBY_BUF) as *mut u8;
        let mut off: usize = 0;
        off += write_bytes(base, off, PREFIX);
        off += write_u64(base, off, now as u64);
        off += write_bytes(base, off, SUFFIX);
        // i32 static 에 직접 대입 (참조 생성 아님).
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

// ─── ABI export: id / display_name (정적) ───

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

// ─── ABI export: lobby 카드 (동적, init 이 구축한 버퍼) ───

#[unsafe(no_mangle)]
pub extern "C" fn lobby_card_ptr() -> i32 {
    // addr_of! 는 참조를 만들지 않으므로 unsafe 불필요.
    core::ptr::addr_of!(LOBBY_BUF) as *const u8 as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn lobby_card_len() -> i32 {
    // raw pointer 의 read_volatile 은 unsafe.
    unsafe { core::ptr::addr_of!(LOBBY_LEN).read_volatile() }
}

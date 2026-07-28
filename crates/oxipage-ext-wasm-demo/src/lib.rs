//! WASM 데모 확장 (doc/08 §8.4).
//!
//! `oxipage-wasm` 호스트가 적재하는 최소 코어 WASM 모듈. `Extension` 서브셋 ABI
//! (`id`/`display_name`/`lobby_summary`/`init`)를 export 하고 호스트 capability
//! (`host_now_unix`/`host_log`)를 import 한다. 라우트·마이그레이션·잡은 없다
//! (알려진 한계).
//!
//! 호스트 타겟(aarch64/x86_64)에서는 이 크레이트가 비어 컴파일된다 — 모든 실제
//! 코드는 `#[cfg(target_arch = "wasm32")]` 아래 있다. 이래야 `cargo clippy
//! --workspace`(호스트) 품질 게이트를 통과하면서 wasm 빌드는 별도 수행된다.

#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", allow(clippy::all))]
#![allow(dead_code)]

#[cfg(target_arch = "wasm32")]
mod guest;

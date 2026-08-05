# 14. Rust 네이티브 프리렌더 + 이미지 최적화 — 후속(follow-up) 항목

**상태**: 기능은 main에 머지 완료 (2026-08-05, commit `78ac713`). 이 문서는 머지 시점에
수용한 트레이드오프 / 남겨둔 후속 항목을 기록한다. 각 항목은 "결국 반드시 풀어야 하는
문제"가 아니라 **트리거가 생기면 재검토할 항목**이다 (판단 기준: "지금 아프냐?").

**관련 문서**: `docs/superpowers/specs/2026-08-05-rust-native-prerender-design.md`

## 기능 요약 (무엇이 배송됐나)

- 블로그 본문이 빌드 타임에 `index.html`에 렌더 → 크롤러/no-JS가 본문을 인덱싱 (SEO/첫화면).
- 로컬 media 이미지 → 반응형 WebP 변형 + `srcset`/`width`/`height` (CLS 방지). `image-manifest.json`을
  Rust 프리렌더와 SPA markdown-it 플러그인 양쪽이 소비.
- 단일 바이너리·Node 제로 유지. GH Pages 프로젝트 경로(`/blog/`)와 apex(`/`) 모두 정정.
- 검증: `cargo test --workspace` 219개 통과, release + web 빌드 클린.

---

## F1. Lossy WebP (우선순위: 낮음 — 트리거 기반)

- **문제**: 순수-Rust `image-webp` 0.2.x는 VP8L **lossless-only**. 사진(JPEG)에서 lossless
  WebP는 소스보다 크거나 비슷하고, lossy q80 대비 2~5배 크다.
- **현재**: lossless + 반응형 리사이징. 핵심 이득(모바일에 적정 크기 서빙)은 이미 확보.
- **대안**: `webp` crate(libwebp, C 빌드 의존, 정적 링크) 추가 → `new_lossy(80)`로 교체.
- **비용**: C 빌드 의존 추가 (단일 바이너리는 유지되나 순수-Rust 성격 약화).
- **트리거**: 블로그 이미지 payload가 체감될 때 (Lighthouse LCP, 모바일 데이터).
- **위치**: `crates/oxibuilder-core/src/media.rs`의 `generate` (인코더 호출 1줄 + Cargo.toml dep).

## F2. alt 텍스트 전달 (우선순위: 중간 — 후속 항목 중 가장 가치 있음)

- **문제**: 최적화 이미지의 `alt`가 `""`로 고정. 접근성(WCAG) + 이미지 SEO 손해. 머지 전에는
  SPA markdown-it이 실제 alt를 냈으므로, 최적화 경로에서 **회귀**.
- **현재**: Rust `render_image_open`이 `alt=""` 일괄; SPA `resolveMedia`도 `alt=""`. 비최적화/외부
  이미지는 SPA가 실제 alt를 내므로 프리렌더(빈 alt)와 하이드레이션(실제 alt)이 비대칭.
- **대안**: Rust 쪽은 pulldown-cmark `Start(Image)`~`End(Image)` 사이 `Text` 이벤트를 모아
  `alt`로 emit. SPA 쪽은 markdown-it 이미지 토큰의 alt를 `resolveMedia`에 전달. 양쪽 수정.
- **비용**: 중간 (Rust 이벤트 수집 + SPA 규칙 alt 전달 + 양쪽 테스트 갱신).
- **트리거**: 접근성 요구 또는 이미지 검색 노출 고민 시. 후속 중 가장 먼저 꺼낼 항목.
- **위치**: `crates/oxibuilder-core/src/markdown.rs` + `web/src/shared/image-manifest.ts` + `Markdown.tsx`.

## F3. 640px 미만 이미지 네이티브 변형 (우선순위: 낮음)

- **문제**: 소스가 640px 미만이면 `media::generate`가 빈 `srcset`을 내고, 프리렌더/SPA는
  raw media ref로 fallback (WebP 변환 없음).
- **현재**: 기능 정상 (no panic, raw 서빙 — Task 3의 empty-srcset fallback). 작은 이미지는
  payload/CLS 영향 미미.
- **대안**: `WIDTHS`에 native-width를 포함시켜 최소 1개 변형 보장.
- **비용**: 낮음.
- **판단**: 사실상 "안 고쳐도 되는" 항목. 작은 이미지가 많아지면 재검토.
- **위치**: `crates/oxibuilder-core/src/media.rs`의 `generate`.

## F4. bun:test 타입 정리 (우선순위: 낮음 — 만질 때)

- **문제**: `image-manifest.test.ts`의 `@ts-expect-error`(bun:test import). `bun-types`를 나중에
  추가하면 unused directive(TS2578)가 되어 `tsc --noEmit`이 깨지는 잠재 지뢰.
- **대안**: 테스트 러너 표준화 시 `bun add -d bun-types` + `/// <reference types="bun-types" />`로 교체.
- **비용**: 1분.
- **트리거**: web 테스트 셋업을 만질 때.
- **위치**: `web/src/shared/image-manifest.test.ts`.

---

## 수동 스모크 (기능 확인 방법)

1. 이미지 있는 글 작성 → `oxibuilder build`.
2. `out/blog/<slug>/index.html`에 본문 텍스트 + `<img srcset width height>`가 있는지 확인.
3. `out/media/_derived/*.webp` + `out/data/image-manifest.json` 존재 확인.
4. `oxibuilder console --preview`로 브라우저에서 글 열어 최적화 이미지 로딩 + SPA 하이드레이션 확인.

## 판단 기준

F2(alt)만 "언젠가 고칠 가치"가 있는 실질 항목이고, F1/F3/F4는 트리거가 생길 때만.
기준은 언제나 **"지금 아프냐?"** — 아프지 않으면 건드리지 않는다.

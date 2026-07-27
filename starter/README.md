# oxipage-starter

원클릭 설치 템플릿 저장소 (doc/05 §5.7, doc/07 §7.7). 이 디렉토리는 별도 GitHub
저장소(`oxipage-starter`)로 분리할 예정 — 여기엔 설치 스크립트와 템플릿 안내만 둔다.

## 빠른 시작 (curl … | sh)

```bash
curl -fsSL https://raw.githubusercontent.com/oxipage/oxipage-starter/main/install.sh | sh
```

`install.sh`가 하는 일:
1. Rust toolchain 확인 (`rustup`) + `cargo`.
2. `git clone oxipage-starter` → 작업 디렉토리.
3. `cp oxipage.toml.example oxipage.toml` + 대화형으로 `[site]`/`[integrations]` 채움.
4. `cargo build --release -p oxipage-server` (또는 릴리스 바이너리 다운로드).
5. `OXIPAGE_ADMIN_TOKEN=$(openssl rand -hex 32) ./oxipage-core serve` 기동 안내.
6. 첫 PAT 발급: `OXIPAGE_TOKEN=<admin> oxipage auth token create --label owner --scope admin`.

## 템플릿 내용 (별도 저장소 분리 시)

```
oxipage-starter/
├── install.sh
├── oxipage.toml.example     # 이 저장소 루트의 것과 동일
├── README.md                # 5분 시작 가이드
├── .gitignore               # data/, oxipage.db, secrets
└── deploy/                  # Caddyfile/plist/service 예시 (이 저장소 deploy/와 동일)
```

## 현재 상태

Phase 5 OSS 제품화의 일부. 메인 저장소의 `oxipage.toml.example`, `deploy/`,
`docs/extension-sdk.md`, `registry/index.json`이 템플릿 소스. 별도 저장소 분리는
릴리스 시점에 진행.

## 알려진 한계

- v1은 컴파일 타임 정적 링크만 지원 → `oxipage extension install`이 런타임 확장을
  설치하지 못함 (doc/01 §1.4). starter 템플릿은 "전체 확장 포함 빌드"만 제공.
- WASM 컴포넌트 기반 런타임 로딩은 별도 스파이크 (doc/07 §7.7).

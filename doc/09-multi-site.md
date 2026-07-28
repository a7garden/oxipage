# 9장 — 멀티 사이트 (Site Profiles)

## 9.1 동기

oxipage는 순수 HTTP 기반 CLI (§4.1)로 설계되어, 하나의 CLI 바이너리로 어느 oxipage 서버 인스턴스든 원격 관리가 가능하다. 그러나 CLI의 엔드포인트·토큰 해상(resolution)은 현재 **단일 서버**만을 상정한다:

- `--endpoint` / `OXIPAGE_ENDPOINT` → 하나의 엔드포인트
- `~/.config/oxipage/credentials` → 하나의 토큰
- `oxipage.toml`의 `[site].base_url` → 하나의 기본 URL

자기 PC의 self-host 인스턴스, 알리바바 클라우드 VM, fly.io 앱 등 여러 oxipage 사이트를 관리하는 사용자는 **매 명령마다 `--endpoint` + `--token`을 일일이 지정**해야 한다. 이 장은 "사이트(site)" 개념을 도입해 이 문제를 해결한다.

### 예시 시나리오

```
oxipage site add selfhost --endpoint http://localhost:8787 --token X
oxipage site add alibaba --endpoint https://blog.alibaba.com --token Y
oxipage site add flyio --endpoint https://oxipage.fly.dev --token Z

oxipage site use alibaba
oxipage blog publish "hello"          # → alibaba 서버로 발행
oxipage --site flyio status           # → per-command override
```

## 9.2 개념

**사이트(Site)** 는 명명된 접속 프로필이다. 하나의 사이트는 다음을 묶는다:

| 필드 | 필수 | 설명 |
|------|------|------|
| `name` | O | CLI에서 식별자로 쓸 고유 이름. kebab-case 권장 |
| `endpoint` | O | 서버의 base URL. `https://...` 또는 `http://localhost:PORT` |
| `token` | X | Bearer 토큰. 없으면 쓰기 명령은 인증 실패 |

### 사이트가 아닌 것

- 사이트는 `oxipage.toml` 설정과 무관하다 — 서버 구성이 아니라 **CLI가 어디로 접속할지**만 결정한다
- 사이트는 `oxipage serve`의 동작과 무관하다 — `serve`는 로컬 `oxipage.toml`을 읽어 서버를 기동하며, `--site`는 `serve`에 영향을 주지 않는다
- 사이트는 확장이나 콘텐츠 스키마와 무관하다 — 각 서버 인스턴스가 자체적으로 가짐

## 9.3 저장소

### 경로

```
~/.config/oxipage/sites.toml
```

### 포맷

```toml
# ~/.config/oxipage/sites.toml
default_site = "selfhost"

[sites.selfhost]
endpoint = "http://localhost:8787"
token = "oxpat_..."

[sites.alibaba]
endpoint = "https://blog.alibaba.com"
token = "oxpat_..."

[sites.flyio]
endpoint = "https://oxipage.fly.dev"
# token 없음 — 읽기 전용 혹은 OXIPAGE_TOKEN env 주입
```

### 보안

- 파일 권한: **0600** (현재 `~/.config/oxipage/credentials`와 동일)
- 토큰은 평문 저장. PAT 체계(Phase 4, §4.2)가 완비되면 `oxipage auth login`이 OS 키체인을 우선 사용하도록 확장 가능하나, 본 설계 범위는 파일 기반

## 9.4 CLI 서브커맨드

### `oxipage site`

```
oxipage site add <name>   --endpoint <url> [--token <token>] [--default]
oxipage site list                                                       [--json]
oxipage site show    [name]                                             [--json]
oxipage site use     <name>
oxipage site edit    <name> [--endpoint <url>] [--token <token>]
oxipage site rm      <name>
```

### `site add`

```
oxipage site add flyio --endpoint https://oxipage.fly.dev --token oxpat_abc
oxipage site add selfhost --endpoint http://localhost:8787 --default
```

- `--default`: 이 사이트를 기본 사이트로 즉시 설정 (→ `default_site = "selfhost"`)
- 토큰 없는 사이트도 추가 가능 — 읽기 전용 혹은 `OXIPAGE_TOKEN` env 주입용
- 중복 `name`이면 에러 반환. 업데이트는 `site edit`으로

### `site list`

```
$ oxipage site list
* selfhost   http://localhost:8787
  alibaba    https://blog.alibaba.com
  flyio      https://oxipage.fly.dev
```

- `*` = 현재 활성 사이트 (`--site`, `OXIPAGE_SITE`, 또는 `default_site`)
- `--json`: `[{"name":"selfhost","endpoint":"...","active":true,"has_token":true}, ...]`

### `site show`

```
$ oxipage site show
name:       selfhost
endpoint:   http://localhost:8787
token:      oxpat_...abc (masked)

$ oxipage site show alibaba
name:       alibaba
endpoint:   https://blog.alibaba.com
token:      oxpat_...xyz (masked)
```

- `name` 생략 시 현재 활성 사이트
- `--json` 시 토큰은 `{ "token": "oxpat_...abc" }` (마스킹)

### `site use`

```
oxipage site use alibaba
```

→ `sites.toml`의 `default_site = "alibaba"`로 갱신. 이후 명령은 기본적으로 이 사이트를 향함.

### `site edit`

```
oxipage site edit flyio --endpoint https://new.fly.dev
oxipage site edit selfhost --token oxpat_new
```

- 지정한 필드만 갱신. 미지정 필드는 유지
- `--endpoint`만, `--token`만, 혹은 둘 다 가능

### `site rm`

```
oxipage site rm flyio
```

- 활성 사이트(`default_site`)를 삭제하면 `default_site`는 제거되고, 남은 사이트가 있으면 첫 번째가 새 기본값이 됨. 사이트가 없으면 `default_site` 키도 제거

## 9.5 해상 체인 (Resolution Chain)

CLI가 엔드포인트와 토큰을 결정하는 순서:

### 엔드포인트

```
1. --endpoint <url>           (per-command override, 최우선)
2. --site <name> → endpoint    (사이트가 지정한 엔드포인트)
3. OXIPAGE_SITE → endpoint     (env로 지정된 사이트의 엔드포인트)
4. default_site → endpoint     (sites.toml의 기본 사이트)
5. OXIPAGE_ENDPOINT <url>      (legacy env)
6. oxipage.toml [site].base_url (local config)
7. http://127.0.0.1:8787       (hard-coded fallback)
```

### 토큰 (endpoint와 독립적 해상)

엔드포인트가 사이트에서 결정되더라도, 토큰은 **별도 체인**으로 해상한다 — 사이트에 token 필드가 없으면(`None`) 체인이 멈추지 않고 env/credentials로 폴백한다.

```
1. --token <token>             (per-command override, 최우선)
2. --site <name> → token       (사이트에 token이 있으면 사용)
3. OXIPAGE_SITE → token        (env로 지정된 사이트에 token이 있으면 사용)
4. default_site → token        (기본 사이트에 token이 있으면 사용)
5. OXIPAGE_TOKEN <token>       (legacy env — 사이트 token이 없으면 폴백)
6. ~/.config/oxipage/credentials (legacy 파일)
7. none                        (읽기 명령은 OK, 쓰기는 인증 실패)
```

핵심: 사이트가 resolve됐다고 토큰 체인이 종료되지 않는다. 사이트에 token이 없거나 비어있으면 5→6→7로 폴백한다. 이렇게 하면 "flyio 사이트는 token 없이 endpoint만 등록하고, 실제 토큰은 OXIPAGE_TOKEN env로 주입"하는 사용 패턴이 가능하다.

### 특수 규칙

- `--site`와 `--endpoint`/`--token`이 함께 쓰이면: `--site`가 우선 컨텍스트를 설정하되, `--endpoint`/`--token`이 개별 오버라이드로 덮어씀
  ```
  oxipage --site alibaba --endpoint https://staging.example.com blog list
  # → token은 alibaba 사이트의 토큰, endpoint는 staging으로 오버라이드
  ```
- `OXIPAGE_SITE` env가 설정되어 있고 `--site` flag도 있으면 flag 우선
- 사이트가 하나도 없으면 legacy 체인(5→6→7)으로 폴백 → **기존 동작 완전 보존**

## 9.6 기존 명령과의 상호작용

### `oxipage serve`

`serve`는 사이트 개념과 무관하다. 항상 로컬의 `oxipage.toml`을 읽어 서버를 기동한다.

```
oxipage --site alibaba serve     # → 여전히 로컬 oxipage.toml로 서버 기동
```

의도된 동작: 사이트는 "어디로 요청을 보낼지"를 결정하는 것이지 "어디서 서버를 실행할지"를 결정하지 않는다.

### `oxipage init`

`init`도 사이트와 무관. 항상 로컬에 `oxipage.toml`을 스캐폴딩한다.

### `oxipage status`

```
$ oxipage status
site:          selfhost (http://localhost:8787)    ← 현재 활성 사이트 표시
authenticated: yes
extensions:    8 enabled
...
```

`--site`나 `default_site`가 설정되어 있으면 `status` 출력에 사이트 정보가 포함된다.

### `oxipage auth`

```
oxipage --site alibaba auth login
oxipage --site alibaba auth token create --label "omp-agent" --scopes post:write
```

`auth` 명령은 사이트의 엔드포인트를 향해 인증을 수행한다. 토큰은 서버에서 발급되며, CLI는 반환된 토큰을 `sites.toml`의 해당 사이트에 자동 저장하지 않는다. 사용자가 명시적으로:

```
oxipage site edit alibaba --token <발급받은 토큰>
```

## 9.7 구현 영향도

### 변경 파일

| 파일 | 변경 |
|------|------|
| `crates/oxipage-cli/Cargo.toml` | `toml` 의존성 추가 (없으면) |
| `crates/oxipage-cli/src/main.rs` | `--site` global flag 추가 |
| `crates/oxipage-cli/src/commands.rs` | `Command::Site` enum 변형 + `resolve_endpoint`/`resolve_token`에 site 체인 추가 + `site_*` 핸들러 |
| `crates/oxipage-cli/src/sites.rs` | **신규** — `SiteConfig` 구조체, `load_sites`/`save_sites`/`resolve_active` |
| `crates/oxipage-cli/src/credentials.rs` | 변경 없음 (legacy 폴백으로만 사용) |

### 미변경

- `oxipage-core`, `oxipage-server` — CLI 기능이므로 서버·코어 영향 없음
- `oxipage.toml` 스키마 — 사이트는 서버 설정과 독립
- 기존 `credentials.rs` — legacy 경로로 유지

## 9.8 향후 확장 (v2)

본 설계 범위 밖이나, 자연스러운 확장 지점:

1. **`--site`를 `serve`에 연결**: `oxipage site use alibaba && oxipage serve`가 `alibaba` 사이트에 해당하는 원격 `oxipage.toml`을 다운로드해 로컬에서 복제 서버를 띄우는 흐름. 현재는 범위 밖.
2. **OS 키체인 통합**: Phase 4 PAT 체계 도입 시, 토큰을 `sites.toml` 대신 macOS Keychain / freedesktop Secret Service에 저장
3. **`site clone`**: 원격 서버의 `oxipage.toml` + 데이터를 로컬로 복제
4. **`site health`**: `GET /api/v1/health`로 사이트 접속 가능 여부 확인
5. **사이트별 `--config` 경로**: `[sites.selfhost]`에 `config = "/path/to/oxipage.toml"` → `serve`가 자동 선택

## 9.9 레퍼런스

- §4.1 CLI는 API의 레퍼런스 클라이언트
- §4.2 인증 흐름 (PAT 체계)
- §5.x 배포 시나리오 (self-host, cloud VM, fly.io 등)

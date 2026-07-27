# oxipage-starter

One-click install template repo (doc/05 §5.7, doc/07 §7.7). This directory is meant to be split
into a separate GitHub repo (`oxipage-starter`) — for now it holds only the install script outline
and template notes.

## Quick start (curl … | sh)

```bash
curl -fsSL https://raw.githubusercontent.com/oxipage/oxipage-starter/main/install.sh | sh
```

What `install.sh` does:

1. Check the Rust toolchain (`rustup`) + `cargo`.
2. `git clone oxipage-starter` into a working directory.
3. `cp oxipage.toml.example oxipage.toml` and interactively fill in `[site]` / `[integrations]`.
4. `cargo build --release -p oxipage-server` (or download a release binary).
5. Guide you to start it: `OXIPAGE_ADMIN_TOKEN=$(openssl rand -hex 32) ./oxipage-server`.
6. Mint the first PAT: `OXIPAGE_TOKEN=<admin> oxipage auth token create --label owner --scopes admin`.

## Template contents (when split into its own repo)

```
oxipage-starter/
├── install.sh
├── oxipage.toml.example     # identical to the main repo's
├── README.md                # 5-minute quickstart
├── .gitignore               # data/, oxipage.db, secrets
└── deploy/                  # Caddyfile/plist/service examples (same as main repo deploy/)
```

## Current status

Part of Phase 5 OSS productization. The main repo's `oxipage.toml.example`, `deploy/`,
`docs/extension-sdk.md`, and `registry/index.json` are the template sources. Splitting into the
separate repo happens at release time.

## Known limitations

- v1 supports compile-time static linking only, so `oxipage extension install` cannot install a
  runtime extension (doc/01 §1.4). The starter template ships a "full build with all extensions."
- WASM-component-based runtime loading is a separate spike (doc/07 §7.7).

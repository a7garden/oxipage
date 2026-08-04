# oxibuilder-starter

One-click install template repo (doc/05 §5.7, doc/07 §7.7). This directory is meant to be split
into a separate GitHub repo (`oxibuilder-starter`) — for now it holds only the install script outline
and template notes.

## Quick start (curl … | sh)

```bash
curl -fsSL https://raw.githubusercontent.com/oxibuilder/oxibuilder-starter/main/install.sh | sh
```

What `install.sh` does:

1. Check the Rust toolchain (`rustup`) + `cargo`.
2. `git clone oxibuilder-starter` into a working directory.
3. `cp oxibuilder.toml.example oxibuilder.toml` and interactively fill in `[site]` / `[integrations]`.
4. `cargo build --release -p oxibuilder-console` (or download a release binary).
5. Guide you to start it: `OXIBUILDER_ADMIN_TOKEN=$(openssl rand -hex 32) ./oxibuilder-console`.
6. Mint the first PAT: `OXIBUILDER_TOKEN=<admin> oxibuilder auth token create --label owner --scopes admin`.

## Template contents (when split into its own repo)

```
oxibuilder-starter/
├── install.sh
├── oxibuilder.toml.example     # identical to the main repo's
├── README.md                # 5-minute quickstart
├── .gitignore               # data/, oxibuilder.db, secrets
└── deploy/                  # Caddyfile/plist/service examples (same as main repo deploy/)
```

## Current status

Part of Phase 5 OSS productization. The main repo's `oxibuilder.toml.example`, `deploy/`,
`docs/extension-sdk.md`, and `registry/index.json` are the template sources. Splitting into the
separate repo happens at release time.

## Known limitations

- v1 supports compile-time static linking only, so `oxibuilder extension install` cannot install a
  runtime extension (doc/01 §1.4). The starter template ships a "full build with all extensions."
- WASM-component-based runtime loading is a separate spike (doc/07 §7.7).

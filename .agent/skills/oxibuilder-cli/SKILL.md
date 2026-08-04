---
name: oxibuilder-cli
description: >
  CLI skill for publishing content to a self-hosted Oxibuilder personal site. React to requests that
  create or manage content: writing a blog post, adding a project to the portfolio, recording a
  movie/book review, scraping a link, adding ecosystem links. Triggers include phrases like
  "post this to the blog", "add this project to my portfolio", "scrape this link", "publish this".
  The CLI is language-agnostic, so the same commands work regardless of the language the user
  speaks to you in.
---

# Oxibuilder CLI usage

## Principles
- Every `add`/`new` command creates a **draft only**. Never use `--publish` or the `publish`
  command until the user explicitly says "publish" / "게시해줘" / "make it live".
- Always append `--json` to commands so you can parse the output.
- On failure (especially connection refused, 5xx), do **not** retry blindly — report the exact
  error (host/port/HTTP status) and ask the user how to proceed.

## Authentication (local management server)

- The management server (`oxibuilder console`) is **local-only by design** — bound to `127.0.0.1`
  with no auth. Run it on the same machine; do not expose it to the network.
- The CLI still accepts `--token` / `OXIBUILDER_TOKEN` for symmetry with future remote servers,
  but the local server ignores it. Resolution order: `--token` → `OXIBUILDER_TOKEN` env →
  `~/.config/oxibuilder/credentials` (0600) → `sites.toml` (multi-site) → none.
- The `--site <name>` flag (or `OXIBUILDER_SITE` env) selects a remote site from `sites.toml`.
  The token in that profile is sent to the remote console, but **the remote server must enforce
  auth at its own reverse-proxy** — `OXIBUILDER_TOKEN` is not a substitute for that.
- On `401`/`403` from a remote endpoint, **ask the user to check the reverse-proxy setup** —
  do not retry, do not invent tokens.

## Endpoint
- The CLI targets the server at: `--endpoint` / `OXIBUILDER_ENDPOINT` env → `[site].base_url` in
  `oxibuilder.toml` → default `http://127.0.0.1:8787`.

## Currently supported commands

```text
oxibuilder init                                   # scaffold oxibuilder.toml (profile-only, Korean defaults)
oxibuilder status [--json]                         # server + content summary
oxibuilder console [--port 8787]                     # start the local dev server
oxibuilder site list | add | use | show | edit | rm           # multi-site profiles (sites.toml, 0600)
oxibuilder blog new "<title>" [--lang ko|en] [--file DRAFT.md] [--tag t1 --tag t2] [--json]
oxibuilder blog publish <slug> [--json]
oxibuilder blog list [--draft] [--lang ko] [--json]
oxibuilder blog show <slug> [--json]
oxibuilder blog edit <slug> [--title ...] [--file BODY.md] [--tag ...] [--json]
oxibuilder blog rm <slug> [--json]
oxibuilder project add --title-ko "..." --title-en "..." [--desc-ko F] [--desc-en F] \
    [--tech rust --tech react] [--link repo=URL] [--status wip|active|archived] [--featured] [--publish]
oxibuilder project publish <slug>
oxibuilder project list [--status ...]
oxibuilder project show <slug>
oxibuilder link add --title "..." --url "..." [--desc-ko ...] [--desc-en ...] [--featured]
oxibuilder link list
oxibuilder link rm <id>
oxibuilder lobby layout <extension> --mode canvas|grid|list
oxibuilder lobby config [--json]
oxibuilder build [--site <name>] [--json]              # static site generation
oxibuilder deploy [--target github-pages] [--site <name>] [--dry-run] [--json]  # github-pages only; CF/Netlify bail
oxibuilder query "<SQL>" [--json]                      # direct SQL query (read-only)
oxibuilder schema [--extension <name>] [--json]        # DB schema discovery
oxibuilder cache refresh [--extension <name>] [--json] # refresh external API cache
oxibuilder console --preview [--port 8787]               # preview built static site
```

> CLI subcommands for the Phase 2 extensions (`novel`, `review movie`, `review book`, `scrap`,
> `activity sync`) are deferred. Today those are reachable via the HTTP API and the web UI.

## Example workflows

### Write a blog post
1. Turn the user's request into a markdown body and save it to a temp file.
2. `oxibuilder blog new "<title>" --lang en --file <tmpfile> --json`
3. Tell the user the `data.slug` from the result and ask whether to publish.
4. Only on explicit approval: `oxibuilder blog publish <slug> --json`

### Add a project to the portfolio
1. `oxibuilder project add --title-ko "..." --title-en "..." --desc-ko FILE --desc-en FILE --tech rust --tech react --json`
2. Report the resulting slug. Only use `--publish` on explicit user approval.

### Scrape / add a link
1. Given only a URL: `oxibuilder link add --title "..." --url "..." --json`
2. If the user adds a comment, pass it via `--desc-ko` / `--desc-en`.

### Build and deploy the static site
1. After content changes, run `oxibuilder build --json` to regenerate the static site.
2. Run `oxibuilder deploy --target github-pages --json` to publish to GitHub Pages.
3. For local preview first: `oxibuilder console --preview` then open http://127.0.0.1:8787.

### Query content (AI agent SQL access)
1. Check schema: `oxibuilder schema --json`
2. Query: `oxibuilder query "SELECT slug, title FROM blog_posts WHERE tags LIKE '%rust%'" --json`

### Refresh external caches
1. `oxibuilder cache refresh --extension activity --json` (GitHub activity)
2. `oxibuilder cache refresh --extension movies --json` (TMDB poster cache)

## Do not
- Do not generate and publish content the user did not ask for.
- Do not invent ratings or review text without the user's explicit input.
- Do not auto-publish a draft created with `--publish` absent without confirmation.
- Do not attempt hardcoded fake tokens if no token is available.

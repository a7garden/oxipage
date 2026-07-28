---
name: oxipage-cli
description: >
  CLI skill for publishing content to a self-hosted Oxipage personal site. React to requests that
  create or manage content: writing a blog post, adding a project to the portfolio, recording a
  movie/book review, scraping a link, adding ecosystem links. Triggers include phrases like
  "post this to the blog", "add this project to my portfolio", "scrape this link", "publish this".
  The CLI is language-agnostic, so the same commands work regardless of the language the user
  speaks to you in.
---

# Oxipage CLI usage

## Principles
- Every `add`/`new` command creates a **draft only**. Never use `--publish` or the `publish`
  command until the user explicitly says "publish" / "게시해줘" / "make it live".
- Always append `--json` to commands so you can parse the output.
- On failure (especially 403 / insufficient scope), do **not** retry — report to the user exactly
  which permission is missing.

## Authentication (token)

- This skill requires an Oxipage PAT to be **pre-provisioned** in the oh-my-pi environment:
  1. The owner runs, locally: `OXIPAGE_TOKEN=$OXIPAGE_ADMIN_TOKEN oxipage auth token create --label omp-agent --scopes post:write` (issued with the server's `OXIPAGE_ADMIN_TOKEN`, which has `admin` scope. Do **not** grant the agent `post:publish` — draft-first principle).
  2. Inject the plain token into the oh-my-pi environment variable `OXIPAGE_TOKEN`. The plain token is shown only once, so store it immediately.
  3. The CLI auto-attaches `Authorization: Bearer` when `OXIPAGE_TOKEN` is set. Without it, every write command fails with 401.
- **`oxipage auth login` (browser) is not implemented** — it only prints guidance. Instead, issue
  PATs via `oxipage auth token create`, and manage local storage with `oxipage auth set <token>`
  (credentials file, 0600) / `oxipage auth status` / `oxipage auth unset`. PAT scope separation
  (`post:write`/`post:publish`/`read`) is complete as of Phase 4.
- On token expiry or insufficient scope (403), **ask the user to re-issue** — never attempt to
  issue or escalate tokens yourself.

## Endpoint
- The CLI targets the server at: `--endpoint` / `OXIPAGE_ENDPOINT` env → `[site].base_url` in
  `oxipage.toml` → default `http://127.0.0.1:8787`.

## Currently supported commands

```text
oxipage init                                   # scaffold oxipage.toml (profile-only, Korean defaults)
oxipage status [--json]                         # server + content summary
oxipage serve [--port 8787]                     # start the local dev server
oxipage auth set <token>                        # store token in credentials file (0600)
oxipage auth status | unset                      # check / clear stored token
oxipage auth token create --label X --scopes post:write,post:publish   # issue a PAT (needs admin, plain shown once)
oxipage auth token list | revoke <id>            # manage PATs (needs admin)
oxipage blog new "<title>" [--lang ko|en] [--file DRAFT.md] [--tag t1 --tag t2] [--json]
oxipage blog publish <slug> [--json]
oxipage blog list [--draft] [--lang ko] [--json]
oxipage blog show <slug> [--json]
oxipage blog edit <slug> [--title ...] [--file BODY.md] [--tag ...] [--json]
oxipage blog rm <slug> [--json]
oxipage project add --title-ko "..." --title-en "..." [--desc-ko F] [--desc-en F] \
    [--tech rust --tech react] [--link repo=URL] [--status wip|active|archived] [--featured] [--publish]
oxipage project publish <slug>
oxipage project list [--status ...]
oxipage project show <slug>
oxipage link add --title "..." --url "..." [--desc-ko ...] [--desc-en ...] [--featured]
oxipage link list
oxipage link rm <id>
oxipage lobby layout <extension> --mode canvas|grid|list
oxipage lobby config [--json]
oxipage build [--site <name>] [--json]              # static site generation
oxipage deploy [--target github-pages|cloudflare|netlify] [--site <name>] [--json]  # deploy static site
oxipage query "<SQL>" [--json]                      # direct SQL query (read-only)
oxipage schema [--extension <name>] [--json]        # DB schema discovery
oxipage cache refresh [--extension <name>] [--json] # refresh external API cache
oxipage serve --preview [--port 8787]               # preview built static site
```

> CLI subcommands for the Phase 2 extensions (`novel`, `review movie`, `review book`, `scrap`,
> `activity sync`) are deferred. Today those are reachable via the HTTP API and the web UI.

## Example workflows

### Write a blog post
1. Turn the user's request into a markdown body and save it to a temp file.
2. `oxipage blog new "<title>" --lang en --file <tmpfile> --json`
3. Tell the user the `data.slug` from the result and ask whether to publish.
4. Only on explicit approval: `oxipage blog publish <slug> --json`

### Add a project to the portfolio
1. `oxipage project add --title-ko "..." --title-en "..." --desc-ko FILE --desc-en FILE --tech rust --tech react --json`
2. Report the resulting slug. Only use `--publish` on explicit user approval.

### Scrape / add a link
1. Given only a URL: `oxipage link add --title "..." --url "..." --json`
2. If the user adds a comment, pass it via `--desc-ko` / `--desc-en`.

### Build and deploy the static site
1. After content changes, run `oxipage build --json` to regenerate the static site.
2. Run `oxipage deploy --target github-pages --json` to publish to GitHub Pages.
3. For local preview first: `oxipage serve --preview` then open http://127.0.0.1:8787.

### Query content (AI agent SQL access)
1. Check schema: `oxipage schema --json`
2. Query: `oxipage query "SELECT slug, title FROM blog_posts WHERE tags LIKE '%rust%'" --json`

### Refresh external caches
1. `oxipage cache refresh --extension activity --json` (GitHub activity)
2. `oxipage cache refresh --extension movies --json` (TMDB poster cache)

## Do not
- Do not generate and publish content the user did not ask for.
- Do not invent ratings or review text without the user's explicit input.
- Do not auto-publish a draft created with `--publish` absent without confirmation.
- Do not attempt hardcoded fake tokens if no token is available.

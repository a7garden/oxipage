//! Build-time markdown → HTML (pulldown-cmark), parity with the SPA's markdown-it.
//!
//! ## markdown-it ↔ pulldown-cmark parity (Task 7 note)
//!
//! The SPA (`web/src/shared/Markdown.tsx`) renders markdown-it at runtime in
//! the lobby/post/etc. pages; we render pulldown-cmark at build time so
//! JS-disabled / SEO crawlers see the same HTML.
//!
//! **Current SPA config:** `new MarkdownIt({ linkify: true })` — bare markdown-it
//! 14, NO plugins loaded. So tables / strikethrough / task lists / footnotes
//! are NOT active on the SPA side at all.
//!
//! **Current build config:** pulldown-cmark with `ENABLE_TABLES`,
//! `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS`, `ENABLE_FOOTNOTES`. These
//! enable constructs that the SPA-side markdown-it ignores today. The
//! prerender may therefore emit `<table>`, `<del>`, `<input type="checkbox">`,
//! and footnote markup that the SPA's client-side markdown-it would NOT
//! produce on re-render — so JS-disabled readers see richer output than
//! hydrated readers.
//!
//! **That's intentional for v1.** The build-time options are enabled
//! defensively so blog bodies written with any of those constructs prerender
//! sensibly; the SPA doesn't enable the matching plugins because nobody
//! currently authors tables/strikethrough/etc. in the lobby. If the SPA
//! starts loading `markdown-it-gfm-table` / `markdown-it-task-lists` /
//! `markdown-it-footnote` plugins in the future, this comment is the place
//! to re-balance the pulldown-cmark options to match exactly.
//!
//! **Known intentional delta — linkify.** The SPA's `linkify: true` rewrites
//! bare `https://example.com` into `<a href="...">`. pulldown-cmark doesn't
//! autolink, so the prerendered HTML preserves the literal text. The SPA's
//! client-side hydration then re-renders the same body through markdown-it
//! and the link wraps itself post-hydration; JS-disabled readers see the
//! bare URL but it's still readable.
//!
//! ## Image handling (Task 3)
//!
//! For each markdown image whose URL is a `media/...` ref present in the
//! supplied `ImageManifest`, emits an optimized `<img>` with
//! `src`/`srcset`/`width`/`height`/`loading="lazy"`/`decoding="async"`.
//! All other images (external URLs, media refs not in the manifest) fall
//! back to a plain `<img src="...">` under `asset_base`. The attribute set
//! mirrors `web/src/shared/image-manifest.ts`'s `resolveMedia` so the SPA
//! plugin's hydration pass produces the same DOM.
//!
//! ## `alt=""` v1 simplification
//!
//! `alt=""` is emitted unconditionally for every prerendered image — alt
//! text isn't forwarded from the pulldown-cmark event stream here. The SPA
//! side does the same (the markdown-it image rule never sets an `alt`
//! attribute either), so the hydration diff is null and SEO crawlers see
//! a consistent (empty) alt across both render paths. Re-enable alt
//! forwarding in a follow-up: thread the `Tag::Image { title, .. }`
//! payload through the rewrite pass and emit `alt="{...}"` (HTML-escaped)
//! in `render_image_open`. The SPA rule in
//! `web/src/shared/Markdown.tsx` will need the same change for parity.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::media::{ImageEntry, ImageManifest, ImageSrc};

/// Sentinel for the image-asset base used during prerender. The real
/// `deployment_base` (e.g. `/blog/`) is substituted by the build output
/// writer (Task 5). Using a non-URL sentinel here keeps `markdown::render`
/// ignorant of the page's eventual URL while the body is being built.
pub const BASE_PLACEHOLDER: &str = "\u{0}BASE\u{0}";

/// Render owner-authored markdown to trusted HTML (no sanitization; doc §0.3).
/// `asset_base` rewrites logical `media/...` refs; `images` (Task 3) adds srcset/dims.
pub fn render(md: &str, asset_base: &str, images: &ImageManifest) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    // Collect events first so we can substitute image Starts with `Event::Html`
    // and drop the matching inner alt-text events (and the End(Image) no-op),
    // then hand the whole rewritten stream to `push_html` at once. Splitting the
    // stream across multiple `push_html` calls would lose table/heading nesting
    // state and corrupt unrelated output (e.g. `<th>` vs `<td>` for body cells).
    let events: Vec<Event<'_>> = Parser::new_ext(md, opts).collect();
    let mut out_events: Vec<Event<'_>> = Vec::with_capacity(events.len());
    let mut skip_depth: u32 = 0;
    for ev in events {
        if skip_depth > 0 {
            // Drop everything inside the image span, including the End(Image)
            // no-op (pulldown-cmark's html writer treats it as a no-op anyway).
            match &ev {
                Event::Start(Tag::Image { .. }) => skip_depth += 1,
                Event::End(TagEnd::Image) => skip_depth -= 1,
                _ => {}
            }
            continue;
        }
        if let Event::Start(Tag::Image { dest_url, .. }) = ev {
            let tag = render_image_open(&dest_url, asset_base, images);
            out_events.push(Event::Html(tag.into()));
            skip_depth = 1;
        } else {
            out_events.push(ev);
        }
    }
    let mut out = String::with_capacity(md.len() * 2);
    pulldown_cmark::html::push_html(&mut out, out_events.into_iter());
    out
}

/// Build the `<img ...>` tag for a markdown image.
///
/// - `dest_url`: the raw destination URL from `Tag::Image` (logical or external).
/// - `asset_base`: site-relative base like `/` or `/blog/`; stripped of leading
///   and trailing `/` when prepended to logical refs.
/// - `images`: build-time manifest; when `dest_url` is a known `media/...` ref,
///   emit the full responsive tag, otherwise fall back to a plain `<img src>`
///   under `asset_base` (or untouched for absolute/external URLs).
fn render_image_open(dest_url: &str, asset_base: &str, images: &ImageManifest) -> String {
    let logical = dest_url.trim_start_matches('/');
    let prefix = asset_base.trim_matches('/');
    if let Some(entry) = images.get(logical)
        && !entry.srcset.is_empty()
    {
        // Manifest hit with non-empty srcset — emit the full responsive tag.
        let chosen = pick_src(entry);
        let src = format!("{}/{}", prefix, chosen.url);
        let srcset = render_srcset(&entry.srcset, prefix);
        format!(
            r#"<img src="{src}" srcset="{srcset}" width="{w}" height="{h}" loading="lazy" decoding="async" alt="" />"#,
            src = src,
            srcset = srcset,
            w = entry.width,
            h = entry.height,
        )
    } else if is_media_ref(logical) {
        // Local `media/...` ref that didn't make it into the manifest
        // (missing source, decode failure, etc.). Resolve under asset_base so
        // the browser can still request it; no srcset because we have no dims.
        let src = format!("{}/{}", prefix, logical);
        format!(r#"<img src="{src}" alt="" />"#)
    } else {
        // Absolute URL or non-media ref (http(s)://, data:, mailto:, etc.) —
        // emit untouched, no asset_base prefix.
        format!(r#"<img src="{dest}" alt="" />"#, dest = dest_url)
    }
}

/// `true` when `url` is a site-local media ref (`media/...`).
fn is_media_ref(url: &str) -> bool {
    url.starts_with("media/")
}

/// Pick the `src` for the responsive `<img>`: the variant with the largest
/// width at most 960 px, falling back to the largest available variant if
/// every variant is wider than 960.
fn pick_src(entry: &ImageEntry) -> &ImageSrc {
    let mut best: Option<&ImageSrc> = None;
    for s in &entry.srcset {
        if s.w <= 960 && (best.is_none() || s.w > best.unwrap().w) {
            best = Some(s);
        }
    }
    best.unwrap_or_else(|| entry.srcset.last().expect("ImageEntry.srcset is non-empty"))
}

/// Render the `srcset` attribute: comma-joined `{prefix}/{url} {w}w` descriptors.
/// `prefix` is the already-trimmed `asset_base` (no surrounding slashes); the
/// `/` separator is always emitted, which yields `{prefix}/media/...` and the
/// bare `media/...` when `prefix` is empty (asset_base = `/`).
fn render_srcset(srcset: &[ImageSrc], prefix: &str) -> String {
    let mut parts = Vec::with_capacity(srcset.len());
    for s in srcset {
        parts.push(format!("{}/{} {}w", prefix, s.url, s.w));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{ImageEntry, ImageManifest, ImageSrc};

    #[test]
    fn renders_heading_paragraph_and_code() {
        let md = "# Title\n\npara with `code`\n\n```\nx = 1\n```";
        let html = render(md, "/blog/", &ImageManifest::default());
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<pre"));
    }

    #[test]
    fn renders_table() {
        let md = "| a | b |\n| - | - |\n| 1 | 2 |";
        let html = render(md, "/", &ImageManifest::default());
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn media_image_in_manifest_gets_srcset_and_dims() {
        let mut m = ImageManifest::default();
        m.entries.insert(
            "media/shot.png".into(),
            ImageEntry {
                width: 2000,
                height: 1125,
                srcset: vec![
                    ImageSrc {
                        w: 640,
                        url: "media/_derived/ab-640.webp".into(),
                    },
                    ImageSrc {
                        w: 960,
                        url: "media/_derived/ab-960.webp".into(),
                    },
                    ImageSrc {
                        w: 1280,
                        url: "media/_derived/ab-1280.webp".into(),
                    },
                    ImageSrc {
                        w: 1920,
                        url: "media/_derived/ab-1920.webp".into(),
                    },
                ],
            },
        );
        let html = render("![alt](media/shot.png)", "/blog/", &m);
        assert!(
            html.contains(r#"src="blog/media/_derived/ab-960.webp""#),
            "{html}"
        );
        assert!(html.contains("srcset="));
        assert!(html.contains(r#"width="2000""#));
        assert!(html.contains(r#"height="1125""#));
        assert!(html.contains(r#"loading="lazy""#));
        assert!(html.contains(r#"decoding="async""#));
        assert!(html.contains("ab-640.webp 640w"), "{html}");
    }

    #[test]
    fn empty_srcset_manifest_hit_falls_back_to_plain_img() {
        // Manifest hit with an empty `srcset` (e.g. source image narrower than
        // every configured width — Task 2's `media::generate` produces this for
        // icons/logos/thumbnails). Must NOT panic; must fall through to the
        // plain media-ref branch and emit a bare `<img src>` under `asset_base`.
        let mut m = ImageManifest::default();
        m.entries.insert(
            "media/icon.png".into(),
            ImageEntry {
                width: 32,
                height: 32,
                srcset: vec![],
            },
        );
        let html = render("![x](media/icon.png)", "/blog/", &m);
        assert!(html.contains(r#"src="blog/media/icon.png""#), "{html}");
        assert!(
            !html.contains("srcset="),
            "should not emit srcset when srcset is empty: {html}"
        );
    }

    #[test]
    fn external_image_passes_through() {
        let html = render("![x](https://e.com/a.png)", "/", &ImageManifest::default());
        assert!(html.contains(r#"src="https://e.com/a.png""#));
        assert!(!html.contains("srcset="));
    }
}

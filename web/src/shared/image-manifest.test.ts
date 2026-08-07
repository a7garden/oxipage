// Bun test API is compatible with vitest: describe/it/expect are globals
// when invoked via `bun test`.
// @ts-expect-error — `bun:test` types ship with bun itself, not via a package;
// running `bun test` resolves the globals at runtime. tsc only complains because
// bun-types isn't a declared dep (it isn't needed at runtime for `bun test`).
import { describe, it, expect } from "bun:test";
import { isMediaRef, isOptimizableRef, resolveMedia, type ImageManifest } from "./image-manifest";
import { pickVariant } from "./useOptimizedImage";

// `base` is the deployment `<base href>` WITH its slashes (see
// `deploymentBasePrefix`). `resolveMedia` concatenates `${base}${url}` so the
// emitted `<img>` URLs are absolute (e.g. `/blog/media/_derived/...` under a
// project deploy, `/media/_derived/...` under apex) — the browser merges them
// against `<base href>` unchanged, avoiding the doubling that would happen
// if we emitted a relative URL.

describe("resolveMedia", () => {
  it("returns optimized img for a manifest media ref", () => {
    const m: ImageManifest = {
      "media/shot.png": {
        width: 2000,
        height: 1125,
        srcset: [{ w: 960, url: "media/_derived/ab-960.webp" }],
      },
    };
    const out = resolveMedia("media/shot.png", "/blog/", m);
    expect(out).not.toBeNull();
    expect(out).toContain('src="/blog/media/_derived/ab-960.webp"');
    expect(out).toContain('width="2000"');
    expect(out).toContain("srcset=");
  });

  it("returns null for external urls", () => {
    expect(resolveMedia("https://e.com/a.png", "/", {})).toBeNull();
  });

  it("emits srcset with width descriptors and matching attribute set", () => {
    const m: ImageManifest = {
      "media/x.jpg": {
        width: 2000,
        height: 1125,
        srcset: [
          { w: 640, url: "media/_derived/x-640.webp" },
          { w: 960, url: "media/_derived/x-960.webp" },
          { w: 1280, url: "media/_derived/x-1280.webp" },
          { w: 1920, url: "media/_derived/x-1920.webp" },
        ],
      },
    };
    const out = resolveMedia("media/x.jpg", "/blog/", m);
    expect(out).not.toBeNull();
    expect(out).toContain('width="2000"');
    expect(out).toContain('height="1125"');
    expect(out).toContain('loading="lazy"');
    expect(out).toContain('decoding="async"');
    expect(out).toContain('alt=""');
    expect(out).toContain("/blog/media/_derived/x-640.webp 640w");
    expect(out).toContain("/blog/media/_derived/x-960.webp 960w");
    expect(out).toContain("/blog/media/_derived/x-1280.webp 1280w");
    expect(out).toContain("/blog/media/_derived/x-1920.webp 1920w");
    // pickSrc: largest width ≤ 960 → 960
    expect(out).toContain('src="/blog/media/_derived/x-960.webp"');
  });

  it("returns null for unknown media refs", () => {
    expect(resolveMedia("media/missing.png", "/", {})).toBeNull();
  });

  it("picks the largest available variant when all variants are wider than 960", () => {
    const m: ImageManifest = {
      "media/x.jpg": {
        width: 4000,
        height: 3000,
        srcset: [
          { w: 1280, url: "media/_derived/x-1280.webp" },
          { w: 1920, url: "media/_derived/x-1920.webp" },
        ],
      },
    };
    const out = resolveMedia("media/x.jpg", "/blog/", m);
    expect(out).toContain('src="/blog/media/_derived/x-1920.webp"');
  });

  it("accepts a leading-slash media ref and matches by stripped key", () => {
    const m: ImageManifest = {
      "media/x.jpg": {
        width: 800,
        height: 600,
        srcset: [{ w: 960, url: "media/_derived/x-960.webp" }],
      },
    };
    expect(resolveMedia("/media/x.jpg", "/blog/", m)).not.toBeNull();
  });

  it("emits absolute /-prefixed src when base is '/' (apex deployment)", () => {
    const m: ImageManifest = {
      "media/x.jpg": {
        width: 100,
        height: 50,
        srcset: [{ w: 640, url: "media/_derived/x-640.webp" }],
      },
    };
    const out = resolveMedia("media/x.jpg", "/", m);
    expect(out).toContain('src="/media/_derived/x-640.webp"');
    expect(out).toContain("/media/_derived/x-640.webp 640w");
  });

  it("returns null for a manifest hit with empty srcset (icons/thumbnails)", () => {
    // Parity with Rust's `empty_srcset_manifest_hit_falls_back_to_plain_img`
    // (crates/oxibuilder-core/src/markdown.rs). Task 2's media::generate
    // skips widths larger than the source, so a 32x32 icon lands in the
    // manifest with srcset=[] — resolveMedia must fall through so
    // Markdown.tsx emits the plain `<img src>` via the resolver path.
    const m: ImageManifest = {
      "media/icon.png": { width: 32, height: 32, srcset: [] },
    };
    expect(resolveMedia("media/icon.png", "/blog/", m)).toBeNull();
  });
});

// `isMediaRef` is kept as an alias for the markdown-it rule and admin editor
// (both still call the narrower predicate on purpose — external URLs go
// through `useOptimizedImage`, not the markdown image rule). The new
// `isOptimizableRef` widens it to http(s) too, which is what the SPA hook
// needs for TMDB / Aladin covers.
describe("isMediaRef / isOptimizableRef", () => {
  it("isMediaRef matches logical media refs only", () => {
    expect(isMediaRef("media/foo.png")).toBe(true);
    expect(isMediaRef("/media/foo.png")).toBe(true);
    expect(isMediaRef("https://image.tmdb.org/t/p/w500/x.jpg")).toBe(false);
    expect(isMediaRef("data:image/png;base64,abc")).toBe(false);
  });

  it("isOptimizableRef matches media refs AND http(s) urls", () => {
    expect(isOptimizableRef("media/foo.png")).toBe(true);
    expect(isOptimizableRef("/media/foo.png")).toBe(true);
    expect(isOptimizableRef("https://image.tmdb.org/t/p/w500/x.jpg")).toBe(true);
    expect(isOptimizableRef("http://covers.example.com/p.jpg")).toBe(true);
    expect(isOptimizableRef("data:image/png;base64,abc")).toBe(false);
    expect(isOptimizableRef("ftp://x/y")).toBe(false);
    expect(isOptimizableRef("")).toBe(false);
  });
});

// `pickVariant` is the consumer-side variant selector used by MovieCard /
// MovieDetailPage / BookCard to pick the default `src` for the optimized
// <img>. Mirrors the Rust pick_src (≤960 wins, else largest).
describe("pickVariant", () => {
  it("returns the largest variant whose width is ≤ 960", () => {
    const entry = {
      width: 2000,
      height: 1125,
      srcset: [
        { w: 640, url: "x-640.webp" },
        { w: 1280, url: "x-1280.webp" },
        { w: 1920, url: "x-1920.webp" },
      ],
    };
    expect(pickVariant(entry).w).toBe(640);
  });

  it("falls back to the largest variant when all are wider than 960", () => {
    const entry = {
      width: 2000,
      height: 1125,
      srcset: [
        { w: 1280, url: "x-1280.webp" },
        { w: 1920, url: "x-1920.webp" },
      ],
    };
    expect(pickVariant(entry).w).toBe(1920);
  });

  it("returns the only entry for a single-variant srcset", () => {
    const entry = {
      width: 800,
      height: 600,
      srcset: [{ w: 640, url: "x-640.webp" }],
    };
    expect(pickVariant(entry).url).toBe("x-640.webp");
  });
});

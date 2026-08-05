//! Build-time local-image optimization (Task 2).
//!
//! Staging layout (under `staging_dir`, OUTSIDE `out/`):
//!   `staging_dir/media/_derived/{sha8}-{w}.webp`  — WebP variants
//!   `staging_dir/media/_derived/.cache.json`      — content-hash cache
//! Task 5 copies `staging_dir/media/_derived/` into `out/media/_derived/` after
//! `write_build_output` wipes `out/`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView, ImageEncoder};
use sha2::{Digest, Sha256};

const WIDTHS: [u32; 4] = [640, 960, 1280, 1920];

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImageManifest {
    /// logical `media/...` path → entry
    pub entries: std::collections::HashMap<String, ImageEntry>,
}

impl ImageManifest {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn get(&self, path: &str) -> Option<&ImageEntry> {
        self.entries.get(path)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageEntry {
    pub width: u32,
    pub height: u32,
    pub srcset: Vec<ImageSrc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageSrc {
    pub w: u32,
    pub url: String,
}

/// Decode local `media/...` refs and write WebP variants to `staging_dir`.
///
/// `staging_dir` is OUTSIDE `out/`; the build pipeline copies the derived tree
/// into `out/` after `write_build_output` wipes it. Missing or undecodable
/// refs are skipped (logged via `tracing`), never errored.
pub fn optimize(
    refs: &[String],
    media_dir: &Path,
    staging_dir: &Path,
) -> io::Result<ImageManifest> {
    let derived = staging_dir.join("media").join("_derived");
    std::fs::create_dir_all(&derived)?;
    let cache_path = derived.join(".cache.json");
    let mut cache: HashMap<String, Vec<ImageSrc>> = read_cache(&cache_path);

    let mut manifest = ImageManifest::empty();
    for raw in refs {
        let logical = raw.trim_start_matches('/');
        if !logical.starts_with("media/") {
            continue; // external/non-media: skip
        }
        let src = media_dir.join(logical.trim_start_matches("media/"));
        if !src.exists() {
            continue; // missing: skip, don't error
        }
        let bytes = match std::fs::read(&src) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(ref = %logical, error = %e, "media::optimize: read failed, skipping");
                continue;
            }
        };
        let sha8 = hex8(&Sha256::digest(&bytes));
        let key = format!("{logical}:{sha8}");

        // Cache hit only counts if every variant file actually exists on disk.
        let cached_ok = cache
            .get(&key)
            .filter(|v| v.iter().all(|s| derived.join(url_file(&s.url)).exists()))
            .cloned();

        let entry = match cached_ok {
            Some(srcset) => {
                // Re-derive (w,h) from source bytes so the manifest carries dims
                // even on cache hits (cheap; bytes already in hand).
                decode_dims_and_entry(&bytes, srcset).unwrap_or_else(|| {
                    // Should not happen: cache hit means decode previously succeeded.
                    // But be defensive — fall back to a no-op entry with only width/height.
                    match decode_dims(&bytes) {
                        Some((w, h)) => ImageEntry {
                            width: w,
                            height: h,
                            srcset: Vec::new(),
                        },
                        None => ImageEntry {
                            width: 0,
                            height: 0,
                            srcset: Vec::new(),
                        },
                    }
                })
            }
            None => match generate(&bytes, &sha8, &derived) {
                Ok(e) => {
                    cache.insert(key, e.srcset.clone());
                    e
                }
                Err(e) => {
                    tracing::warn!(ref = %logical, error = %e, "media::optimize: generate failed, skipping");
                    continue;
                }
            },
        };

        manifest.entries.insert(logical.to_string(), entry);
    }

    write_cache(&cache_path, &cache)?;
    Ok(manifest)
}

// --- helpers ---

/// First 8 hex chars of a SHA-256 digest. Used to namespace derived files so
/// distinct sources never collide and unchanged sources hit the cache.
fn hex8(digest: &[u8]) -> String {
    let mut s = String::with_capacity(8);
    for b in &digest[..4] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Map a public URL (`media/_derived/abc-640.webp`) to its disk path under the
/// derived root. The URL is always of the form we wrote, so a simple strip is
/// sufficient and we never trust caller-provided path traversal.
fn url_file(url: &str) -> PathBuf {
    let stripped = url.strip_prefix("media/_derived/").unwrap_or(url);
    PathBuf::from(stripped)
}

fn read_cache(path: &Path) -> HashMap<String, Vec<ImageSrc>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn write_cache(path: &Path, cache: &HashMap<String, Vec<ImageSrc>>) -> io::Result<()> {
    // Atomic-ish write: write to a sibling tmp, then rename. Avoids leaving a
    // half-written .cache.json if the build crashes mid-write.
    let tmp = path.with_extension("json.tmp");
    let json =
        serde_json::to_vec(cache).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)
}

/// Build the entry for a cache hit: re-decode only to recover (w,h); srcset
/// comes from the cache. Returns `None` if decode fails (the bytes are
/// corrupt — caller falls back to a defensive empty entry).
fn decode_dims_and_entry(bytes: &[u8], srcset: Vec<ImageSrc>) -> Option<ImageEntry> {
    let (w, h) = decode_dims(bytes)?;
    Some(ImageEntry {
        width: w,
        height: h,
        srcset,
    })
}

fn decode_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    Some((img.width(), img.height()))
}

/// Decode source bytes, then resize to each target width (capped at source
/// width) and write a WebP variant. Returns the entry on success.
///
/// Aspect ratio: target height is computed from the source aspect so every
/// generated variant is exactly `w` pixels wide (deterministic file names +
/// predictable srcset layout). Resampler: Lanczos3 — best general-purpose
/// downscaler in `image` for photographic content.
fn generate(bytes: &[u8], sha8: &str, derived: &Path) -> io::Result<ImageEntry> {
    let src: DynamicImage = image::load_from_memory(bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("decode: {e}")))?;
    let (w0, h0) = src.dimensions();
    if w0 == 0 || h0 == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty image"));
    }

    let mut srcset = Vec::with_capacity(WIDTHS.len());
    for &w in &WIDTHS {
        if w > w0 {
            continue; // never upscale: skip widths larger than the source
        }
        // Preserve aspect: h = round(h0 * w / w0). Always at least 1px.
        let h = ((h0 as u64 * w as u64 + (w0 as u64 / 2)) / w0 as u64).max(1) as u32;
        let resized = src.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
        let out_path = derived.join(format!("{sha8}-{w}.webp"));

        let file = std::fs::File::create(&out_path)?;
        let mut writer = std::io::BufWriter::new(file);
        // TODO(lossy): switch to lossy q80 once a VP8 encoder is in deps.
        // `image-webp` 0.2.x is VP8L lossless-only — `WebPEncoder::new`'s doc
        // says "Only supports VP8L lossless encoding" and `EncoderParams`
        // has no quality knob. So this emits lossless WebP: ~2-5x larger than
        // lossy q80 for photographs (the blog common case), but byte-stable
        // for fixed input (cache hits stay free). To gain lossy:
        //   (a) add the `webp` crate (libwebp C, statically linked via
        //       image's `webp-sys` path) — proven, pulls a C build dep; or
        //   (b) add a pure-Rust VP8 still-image crate (e.g. `gamut-webp`)
        //       — unproven at our scale, would need benchmarking.
        // Deferred to an explicit decision once real photo sizes bite.
        let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut writer);
        encoder
            .write_image(
                resized.as_bytes(),
                resized.width(),
                resized.height(),
                resized.color().into(),
            )
            .map_err(|e| io::Error::other(format!("webp encode: {e}")))?;
        drop(writer); // flush BufWriter before any later reader touches the file

        srcset.push(ImageSrc {
            w,
            url: format!("media/_derived/{sha8}-{w}.webp"),
        });
    }

    Ok(ImageEntry {
        width: w0,
        height: h0,
        srcset,
    })
}

// --- tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::path::PathBuf;

    fn write_test_png(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
        let p = dir.join(name);
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(w, h, Rgba([255, 0, 0, 255]));
        img.save(&p).unwrap();
        p
    }

    #[test]
    fn optimizes_local_image_to_webp_variants_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let media = tmp.path().join("media");
        std::fs::create_dir_all(&media).unwrap();
        let staging = tmp.path().join("staging"); // OUTSIDE any out/ — survives wipes
        write_test_png(&media, "shot.png", 2000, 1125);

        let m1 = optimize(&["media/shot.png".into()], &media, &staging).unwrap();
        let e = m1.get("media/shot.png").expect("entry present");
        assert_eq!((e.width, e.height), (2000, 1125));
        // widths capped at source (2000): 640,960,1280,1920 all ≤ 2000
        assert_eq!(e.srcset.len(), 4);
        assert!(staging.join("media/_derived").is_dir());
        assert!(e.srcset.iter().all(|s| s.url.ends_with(".webp")));

        // Bonus: each on-disk variant is a real WebP (RIFF…WEBP magic).
        for s in &e.srcset {
            let p = staging
                .join("media/_derived")
                .join(s.url.trim_start_matches("media/_derived/"));
            let bytes = std::fs::read(&p).unwrap();
            assert_eq!(&bytes[..4], b"RIFF", "variant {} missing RIFF magic", s.url);
            assert_eq!(
                &bytes[8..12],
                b"WEBP",
                "variant {} missing WEBP at offset 8",
                s.url
            );
        }

        // Cache-hit: capture mtime + cache.json content; re-run; both must be unchanged.
        let first_variant_path = staging
            .join("media/_derived")
            .join(e.srcset[0].url.trim_start_matches("media/_derived/"));
        let mt1 = std::fs::metadata(&first_variant_path)
            .unwrap()
            .modified()
            .unwrap();
        let cache_json_1 =
            std::fs::read_to_string(staging.join("media/_derived/.cache.json")).unwrap();
        // sha8 is recomputed the same way the impl does it: SHA-256(source bytes).
        let source_bytes = std::fs::read(media.join("shot.png")).unwrap();
        let sha8 = {
            let digest = sha2::Sha256::digest(&source_bytes);
            let mut s = String::with_capacity(8);
            for b in &digest[..4] {
                s.push_str(&format!("{b:02x}"));
            }
            s
        };
        assert!(
            cache_json_1.contains(&format!("\"media/shot.png:{sha8}\"")),
            "cache.json missing expected key; got: {cache_json_1}"
        );

        std::thread::sleep(std::time::Duration::from_millis(50));
        let m2 = optimize(&["media/shot.png".into()], &media, &staging).unwrap();
        let mt2 = std::fs::metadata(&first_variant_path)
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            mt1, mt2,
            "second run regenerated the variant — cache hit failed (mtime changed)"
        );
        assert_eq!(m2.get("media/shot.png").unwrap().srcset.len(), 4);
        // Cache file still present and the key is preserved.
        let cache_json_2 =
            std::fs::read_to_string(staging.join("media/_derived/.cache.json")).unwrap();
        assert_eq!(
            cache_json_1, cache_json_2,
            "cache.json content drifted across runs"
        );
        assert!(cache_json_2.contains(&format!("\"media/shot.png:{sha8}\"")));
    }

    #[test]
    fn missing_ref_is_skipped_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let m = optimize(
            &["media/ghost.png".into()],
            &tmp.path().join("media"),
            &staging,
        )
        .unwrap();
        assert!(m.get("media/ghost.png").is_none());
    }
}

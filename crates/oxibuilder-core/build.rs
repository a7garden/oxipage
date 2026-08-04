fn main() {
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=../../web/dist-static");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = std::path::Path::new(&manifest_dir);
    let web_dist = root.join("../../web/dist");
    let web_dist_static = root.join("../../web/dist-static");

    // Mode 1: Workspace development — web/dist exists, require complete output.
    if web_dist.exists() || web_dist_static.exists() {
        validate_and_copy(
            &web_dist,
            &root.join("embedded-spa"),
            "admin.html",
            "web/dist",
        );
        validate_and_copy(
            &web_dist_static,
            &root.join("embedded-spa-static"),
            "index.html",
            "web/dist-static",
        );
        // Compute a deterministic SHA-256 revision over web/dist (relative filename
        // + bytes per file, sorted at every directory level) and emit it as both
        // a runtime env (consumed by Task 3's option_env!) and a marker file shipped
        // with published crates so packaged builds see the same value.
        let revision = compute_revision(&web_dist);
        std::fs::write(root.join("embedded-spa/.build-revision"), &revision)
            .expect("write embedded-spa/.build-revision");
        println!("cargo:rustc-env=OXIBUILDER_SPA_REVISION={revision}");
    } else if root.join("embedded-spa").exists() || root.join("embedded-spa-static").exists() {
        // Mode 2: Published crate — packaged embeds must already be populated
        // and carry the revision marker that was baked in at publish time.
        require_packaged(&root.join("embedded-spa"), "admin.html");
        require_packaged(&root.join("embedded-spa"), ".build-revision");
        require_packaged(&root.join("embedded-spa-static"), "index.html");
        let revision = std::fs::read_to_string(root.join("embedded-spa/.build-revision"))
            .expect("read embedded-spa/.build-revision");
        println!("cargo:rustc-env=OXIBUILDER_SPA_REVISION={revision}");
    } else {
        // Fresh development clone: no web build and no packaged embeds yet.
        // Fail with the exact command that produces the required output.
        panic!("no SPA bundle found. Run first: cd web && bun run build && bun run build:static");
    }

    // Registry + WASM (unchanged from existing logic).
    copy_or_stub(
        &root.join("_registry.json"),
        &root.join("../../registry/index.json"),
        b"[]",
    );
    copy_or_stub(
        &root.join("_wasm-demo.wasm"),
        &root.join("../../crates/oxibuilder-ext-wasm-demo/artifacts/wasm-demo.wasm"),
        b"",
    );
}

fn validate_and_copy(src: &std::path::Path, dst: &std::path::Path, required: &str, label: &str) {
    if !src.exists() {
        // If the sibling dist exists but this one doesn't, that's a partial workspace build.
        panic!(
            "{label} not found at {}. Run: cd web && bun run build && bun run build:static",
            src.display()
        );
    }
    if !src.join(required).exists() {
        panic!(
            "{label}/{} is missing. Run: cd web && bun run build && bun run build:static",
            required,
        );
    }
    if dst.exists() {
        let _ = std::fs::remove_dir_all(dst);
    }
    copy_dir(src, dst)
        .unwrap_or_else(|e| panic!("failed to copy {label} to {}: {e}", dst.display()));
}

fn require_packaged(dir: &std::path::Path, required: &str) {
    if !dir.join(required).exists() {
        panic!(
            "packaged embed at {} is missing {}. The crate package is incomplete.",
            dir.display(),
            required
        );
    }
}

fn copy_or_stub(dst: &std::path::Path, src: &std::path::Path, empty: &[u8]) {
    if src.exists() {
        std::fs::copy(src, dst).unwrap_or_else(|e| panic!("failed to copy {}: {e}", src.display()));
    } else if !dst.exists() {
        std::fs::write(dst, empty).unwrap();
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Compute a deterministic SHA-256 revision over a directory: every regular file's
/// relative path (POSIX-style) and bytes are folded into the digest. Directory
/// entries are walked in sorted order so identical builds always yield the same
/// hash. Output is the lowercase hex string that Task 3 reads via
/// `option_env!("OXIBUILDER_SPA_REVISION")`.
fn compute_revision(dir: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};

    let mut entries: Vec<std::fs::DirEntry> = Vec::new();
    collect_sorted_files(dir, &mut entries);

    let mut hasher = Sha256::new();
    for entry in &entries {
        let path = entry.path();
        let rel = path
            .strip_prefix(dir)
            .expect("entry under root")
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(&path).expect("read embed file");
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(&bytes);
        hasher.update(b"\0");
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn collect_sorted_files(dir: &std::path::Path, out: &mut Vec<std::fs::DirEntry>) {
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)
        .expect("read embed dir")
        .map(|e| e.expect("embed entry"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let ty = entry.file_type().expect("entry type");
        if ty.is_dir() {
            collect_sorted_files(&path, out);
        } else if ty.is_file() {
            out.push(entry);
        }
    }
}

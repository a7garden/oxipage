fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = std::path::Path::new(&manifest_dir);

    // 1. Embedded SPA — copy from web/dist or create placeholder
    let embed_dir = root.join("embedded-spa");
    let web_dist = root.join("../../web/dist");
    if web_dist.exists() {
        let _ = std::fs::remove_dir_all(&embed_dir);
        copy_dir(&web_dist, &embed_dir).unwrap_or_else(|e| {
            panic!("failed to copy web/dist to embedded-spa: {e}")
        });
    } else if !embed_dir.exists() {
        std::fs::create_dir_all(&embed_dir).unwrap();
        std::fs::write(
            embed_dir.join("index.html"),
            "<!DOCTYPE html><html lang=ko><head><meta charset=utf-8>\
             <title>Oxipage</title></head><body><div id=root>\
             <p>SPA not embedded — build with `bun run build` in web/</p>\
             </div></body></html>",
        )
        .unwrap();
    }

    // 2. Registry index — copy or create empty
    let registry_json = root.join("_registry.json");
    let registry_src = root.join("../../../registry/index.json");
    if registry_src.exists() {
        std::fs::copy(&registry_src, &registry_json).unwrap_or_else(|e| {
            panic!("failed to copy registry/index.json: {e}")
        });
    } else if !registry_json.exists() {
        std::fs::write(&registry_json, "[]").unwrap();
    }

    // 3. WASM demo artifact — copy from workspace or create empty stub.
    //    `include_bytes!("../_wasm-demo.wasm")` needs the file to exist at
    //    compile time regardless of whether the wasm feature is active.
    //    An empty file never matches a valid wasm magic header (0x00 0x61 0x73 0x6d),
    //    so the feature-gated code that reads it will handle the "not a wasm" case.
    let wasm_demo = root.join("_wasm-demo.wasm");
    let wasm_src = root.join("../../../crates/oxipage-ext-wasm-demo/artifacts/wasm-demo.wasm");
    if wasm_src.exists() {
        std::fs::copy(&wasm_src, &wasm_demo).unwrap_or_else(|e| {
            panic!("failed to copy wasm-demo.wasm: {e}")
        });
    } else if !wasm_demo.exists() {
        std::fs::write(&wasm_demo, b"").unwrap();
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if dst.exists() {
        std::fs::remove_dir_all(dst)?;
    }
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

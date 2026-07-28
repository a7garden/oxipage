fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = std::path::Path::new(&manifest_dir);

    // Embedded Admin SPA — copy from admin-web/dist or create placeholder
    let embed_dir = root.join("embedded-spa");
    let web_dist = root.join("../../admin-web/dist");
    if web_dist.exists() {
        let _ = std::fs::remove_dir_all(&embed_dir);
        copy_dir(&web_dist, &embed_dir).unwrap_or_else(|e| {
            panic!("failed to copy admin-web/dist to embedded-spa: {e}")
        });
    } else if !embed_dir.exists() {
        std::fs::create_dir_all(&embed_dir).unwrap();
        std::fs::write(
            embed_dir.join("index.html"),
            "<!DOCTYPE html><html lang=ko><head><meta charset=utf-8>\
             <title>Oxipage Admin</title></head><body><div id=root>\
             <p>Admin SPA not embedded — build with `bun run build` in admin-web/</p>\
             </div></body></html>",
        )
        .unwrap();
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

//! Build-time local-image optimization (Task 2 fills this in).

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImageManifest {
    /// logical `media/...` path → entry
    pub entries: std::collections::HashMap<String, ImageEntry>,
}

impl ImageManifest {
    pub fn empty() -> Self { Self::default() }
    pub fn get(&self, path: &str) -> Option<&ImageEntry> { self.entries.get(path) }
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

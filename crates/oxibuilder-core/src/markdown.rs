//! Build-time markdown → HTML (pulldown-cmark), parity with the SPA's markdown-it.

use pulldown_cmark::{Options, Parser};

/// Render owner-authored markdown to trusted HTML (no sanitization; doc §0.3).
/// `asset_base` rewrites logical `media/...` refs; `images` (Task 3) adds srcset/dims.
pub fn render(md: &str, _asset_base: &str, _images: &crate::media::ImageManifest) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::with_capacity(md.len() * 2);
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ImageManifest;

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
}

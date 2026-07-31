use oxipage_core::theme::{ALL_THEMES, find_theme, is_known_theme};

#[test]
fn catalog_has_six_themes() {
    assert_eq!(ALL_THEMES.len(), 6);
}

#[test]
fn catalog_contains_required_ids() {
    let ids: Vec<&str> = ALL_THEMES.iter().map(|t| t.id).collect();
    for required in ["paper", "midnight", "sepia", "forest", "neon", "canvas"] {
        assert!(ids.contains(&required), "missing {required}");
    }
}

#[test]
fn find_theme_returns_definition() {
    let t = find_theme("paper").expect("paper exists");
    assert_eq!(t.id, "paper");
    assert_eq!(t.name_en, "Paper");
    assert!(matches!(t.mode, oxipage_core::theme::ThemeMode::Light));
    assert_eq!(t.preview_colors.len(), 4);
}

#[test]
fn unknown_theme_returns_none() {
    assert!(find_theme("atlantis").is_none());
    assert!(!is_known_theme("atlantis"));
}

#[test]
fn duplicate_ids_rejected() {
    let ids: Vec<&str> = ALL_THEMES.iter().map(|t| t.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "catalog has duplicate id");
}

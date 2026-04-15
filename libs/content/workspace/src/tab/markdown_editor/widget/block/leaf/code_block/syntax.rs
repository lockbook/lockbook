use std::io::{BufReader, Cursor};
use std::sync::OnceLock;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect_assets::assets::HighlightingAssets;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static SYNTAX_THEME: OnceLock<Theme> = OnceLock::new();

pub(crate) fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| {
        HighlightingAssets::from_binary()
            .get_syntax_set()
            .unwrap()
            .clone()
    })
}

pub(crate) fn syntax_theme() -> &'static Theme {
    SYNTAX_THEME.get_or_init(|| {
        let theme_bytes = include_bytes!("../../../../assets/placeholders.tmTheme").as_ref();
        let cursor = Cursor::new(theme_bytes);
        let mut buffer = BufReader::new(cursor);
        ThemeSet::load_from_reader(&mut buffer).unwrap()
    })
}

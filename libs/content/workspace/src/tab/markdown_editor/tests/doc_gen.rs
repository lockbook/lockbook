//! Structured generator for well-formed GFM markdown source. Built bottom-up:
//! text leaves → inline tree → leaf blocks → container blocks → doc.

use crate::test_utils::byte_source::ByteSource;

/// Common-frequency ASCII words. Dominates by weight so most shrunken failure
/// outputs stay readable even when the generator can pick complex scripts.
const ASCII_WORDS: &[&str] = &["foo", "bar", "baz", "qux", "hello", "world"];

/// Devanagari (Hindi). Vowel signs, conjuncts, virama-joined clusters — the
/// script that crashed the editor on ad-hoc input.
const HINDI_WORDS: &[&str] = &["नमस्ते", "धन्यवाद", "कैसे", "है", "क्या"];

/// CJK. Wide characters with no inter-word spaces.
const CJK_WORDS: &[&str] = &["你好", "谢谢", "中文", "日本語"];

/// Arabic. Right-to-left, joining behavior, combining marks.
const ARABIC_WORDS: &[&str] = &["مرحبا", "شكرا", "كيف", "حال"];

/// Emoji shapes that historically trip text engines: variation selector, ZWJ
/// sequence, regional-indicator flag.
const EMOJI: &[&str] = &["👋", "🎉", "❤️", "👨\u{200D}👩\u{200D}👧", "🇺🇸"];

const MAX_INLINE_DEPTH: usize = 3;

/// Maximum depth of container-block nesting. Three exercises real-world
/// structures like `> - > foo` (blockquote → list → blockquote → paragraph)
/// without runaway recursion.
const MAX_BLOCK_DEPTH: usize = 3;

/// One arbitrary BMP codepoint (or empty string if the draw lands on a
/// surrogate). The fallback fuzz that lets unexpected codepoints reach the
/// renderer.
fn random_codepoint(src: &mut ByteSource) -> String {
    let cp = src.draw(0x10000) as u32;
    char::from_u32(cp)
        .map(|c| c.to_string())
        .unwrap_or_default()
}

/// One codepoint pulled out of a complex-script word — often a partial
/// grapheme (combining mark, virama, ZWJ). Exactly the shape that trips
/// editors that conflate char positions with grapheme-cluster positions.
pub fn gen_complex_codepoint(src: &mut ByteSource) -> String {
    const SOURCES: &[&str] =
        &["नमस्ते", "धन्यवाद", "कैसे", "क्या", "مرحبا", "شكرا", "كيف", "👨\u{200D}👩\u{200D}👧", "❤️"];
    let word = SOURCES[src.draw(SOURCES.len())];
    let chars: Vec<char> = word.chars().collect();
    chars[src.draw(chars.len())].to_string()
}

/// Code-span body. ASCII only — embedding a backtick would require a longer
/// surrounding fence and we don't model that yet.
fn gen_code_text(src: &mut ByteSource) -> String {
    let n = 1 + src.bias(&[3, 3, 2, 1]);
    (0..n)
        .map(|_| ASCII_WORDS[src.draw(ASCII_WORDS.len())])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Long single token: stresses the section-break path. The first variant
/// fits in a normal row but overflows narrow contexts (table cells, deeply
/// nested blockquotes); the second is too wide for any sane row.
fn gen_long_token(src: &mut ByteSource) -> String {
    match src.bias(&[2, 1]) {
        0 => "https://example.com/very/long/path/to/resource".to_string(),
        _ => "x".repeat(80),
    }
}

/// Filename-shaped identifier token: alphanumeric chunks joined by
/// `_`, `-`, or `.` with no whitespace. Mirrors the real-world
/// attachment names that exposed the walker's lack of break
/// opportunities inside long `_`-joined runs.
pub fn gen_identifier_token(src: &mut ByteSource) -> String {
    let n_chunks = 2 + src.bias(&[2, 3, 2, 2, 1]);
    let mut out = String::new();
    for i in 0..n_chunks {
        if i > 0 {
            // Separator. `.` is a UAX#14 break opportunity (class
            // IS); `_` and `-` are not — keep them dominant so the
            // resulting token has long un-breakable runs.
            let sep = match src.bias(&[4, 4, 1]) {
                0 => '_',
                1 => '-',
                _ => '.',
            };
            out.push(sep);
        }
        let chunk = ASCII_WORDS[src.draw(ASCII_WORDS.len())];
        out.push_str(chunk);
        // Append some digits for filename feel.
        if src.bias(&[1, 1]) == 0 {
            let d = src.draw(10000);
            out.push_str(&d.to_string());
        }
    }
    out.push_str(".png");
    out
}

/// Code language hints for fenced code blocks. Empty string included so
/// we also exercise the "no language" path. `js` / `javascript` are
/// excluded because the bundled "JavaScript (Babel)" grammar uses a `\g`
/// regex backref that panics fancy-regex inside `highlight_line` — the
/// renderer wraps the call in `catch_unwind`, but the panic hook still
/// prints to stderr, and 2048 iterations × N JS blocks would dump MBs of
/// noise. JS coverage lives in its own focused test.
const LANGS: &[&str] = &["", "rust", "python", "text"];

/// HTML block tags from CommonMark's "type 6" start condition (block-level
/// tags that begin a line). Kept short — the parser cares about the tag
/// shape, not the specific element.
const HTML_BLOCK_TAGS: &[&str] = &["div", "section", "article", "aside"];

/// Fenced code block: ``` + optional language + body + ```. Body uses
/// `gen_code_text` (ASCII words) since code blocks render content literally.
/// Occasionally produces a body taller than the 600px test viewport so
/// the cursor-visibility property exercises the "cursor stuck in tall
/// row across keystrokes" path — Newline inside a fenced code block
/// keeps the cursor on the same `CodeBlock` AST row, where the
/// scroll-to-cursor logic that uses "any overlap with viewport" would
/// fail to scroll.
fn gen_fenced_code(src: &mut ByteSource) -> String {
    let lang = LANGS[src.draw(LANGS.len())];
    let n_lines = gen_code_block_line_count(src);
    let body = (0..n_lines)
        .map(|_| gen_code_text(src))
        .collect::<Vec<_>>()
        .join("\n");
    format!("```{lang}\n{body}\n```\n")
}

/// Indented code block: 4-space prefix on each line. Same tall-block
/// reasoning as `gen_fenced_code` — Newline inside this block keeps
/// the cursor on the same row.
fn gen_indented_code(src: &mut ByteSource) -> String {
    let n_lines = gen_code_block_line_count(src);
    (0..n_lines)
        .map(|_| format!("    {}\n", gen_code_text(src)))
        .collect::<String>()
}

/// Line count for a code block: short by default, with a
/// large-but-minority probability of a tall variant (~50 lines, past
/// the 600px test viewport) so the cursor-visibility property reliably
/// exercises the "cursor stuck in tall row across keystrokes" path.
fn gen_code_block_line_count(src: &mut ByteSource) -> usize {
    if src.bias(&[7, 3]) == 1 { 50 } else { 1 }
}

/// Thematic break: 3+ of `-`, `*`, or `_`.
fn gen_thematic_break(src: &mut ByteSource) -> String {
    let kind = match src.bias(&[3, 1, 1]) {
        0 => '-',
        1 => '*',
        _ => '_',
    };
    let count = 3 + src.bias(&[5, 2, 1]);
    let mut s: String = std::iter::repeat_n(kind, count).collect();
    s.push('\n');
    s
}

/// Math block: `$$ ... $$` with literal body (comrak's math extension).
fn gen_math_block(src: &mut ByteSource) -> String {
    let body = gen_code_text(src);
    format!("$$\n{body}\n$$\n")
}

/// Prefixes every line of `s` with `prefix`. Empty lines get the prefix
/// trimmed of trailing whitespace so a `"> "` prefix becomes `">"` on a
/// blank line — keeps the block quote open without inserting trailing
/// whitespace that some markdown linters complain about.
fn prefix_lines(s: &str, prefix: &str) -> String {
    let trimmed = prefix.trim_end();
    s.lines()
        .map(|line| if line.is_empty() { trimmed.to_string() } else { format!("{prefix}{line}") })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Indents every line of `s` after the first by `indent`. Used for list
/// items: the marker stands on the first line, continuation lines align to
/// the marker's content column.
fn indent_continuation(s: &str, indent: &str) -> String {
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(indent);
            }
        }
        out.push_str(line);
    }
    out
}

/// Encodes a `cols`-wide indent as spaces, pure tabs (tab stops every
/// 4 cols), or a tab+space mix. Tab forms require `whitespace.tab`;
/// otherwise pure spaces. Gives list continuation the same encoding
/// variety a hand-indented document has.
fn encode_indent(cols: usize, f: &Features, src: &mut ByteSource) -> String {
    if cols == 0 {
        return String::new();
    }
    if !f.whitespace.tab {
        return " ".repeat(cols);
    }
    match src.bias(&[3, 1, 1]) {
        0 => " ".repeat(cols),
        1 if cols % 4 == 0 => "\t".repeat(cols / 4),
        _ => {
            let tabs = cols / 4;
            "\t".repeat(tabs) + &" ".repeat(cols - tabs * 4)
        }
    }
}

// ─── document generator ───────────────────────────────────────────────
//
// `gen_doc(src, &features)` is the single document generator. Every
// element is gated on its `Features` flag, so callers dial the
// distribution from one engine: `Features::default()` (all off) yields
// plain ASCII paragraphs; `Features::all()` exercises every construct;
// presets in between target a specific axis (e.g. nested lists) for an
// invariant that needs that structure on most seeds.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptFeatures {
    pub hindi: bool,
    pub cjk: bool,
    pub arabic: bool,
    pub emoji: bool,
    pub arbitrary: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WhitespaceFeatures {
    pub tab: bool,
    pub nbsp: bool,
    pub long_token: bool,
    pub id_token: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InlineFeatures {
    pub emph: bool,
    pub strong: bool,
    pub strikethrough: bool,
    pub underline: bool,
    pub highlight: bool,
    pub spoiler: bool,
    pub subscript: bool,
    pub superscript: bool,
    pub code: bool,
    pub math: bool,
    pub link: bool,
    pub autolink: bool,
    pub wikilink: bool,
    pub image: bool,
    pub shortcode: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockFeatures {
    pub atx_heading: bool,
    pub setext_heading: bool,
    pub fenced_code: bool,
    pub indented_code: bool,
    pub thematic_break: bool,
    pub html_block: bool,
    pub table: bool,
    pub math_block: bool,
    pub block_quote: bool,
    pub bullet_list: bool,
    pub ordered_list: bool,
    /// Ordered lists may start at a non-1, possibly multi-digit number,
    /// so marker width varies within one list (`9.`, `10.`, …).
    pub ordered_nonstandard_start: bool,
    pub task_list: bool,
    pub alert: bool,
    pub nested_containers: bool,
    pub multi_line_paragraph: bool,
    /// Sub-lists / continuation indented 1-3 columns past the marker's
    /// content column (valid relative indent per GFM), not flush to it
    pub wide_list_indent: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentFeatures {
    pub front_matter: bool,
    pub crlf: bool,
    pub empty_corner_cases: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub scripts: ScriptFeatures,
    pub whitespace: WhitespaceFeatures,
    pub inlines: InlineFeatures,
    pub blocks: BlockFeatures,
    pub document: DocumentFeatures,
}

impl Features {
    /// All flags on — equivalent to the `gen_doc` distribution.
    pub fn all() -> Self {
        Self {
            scripts: ScriptFeatures {
                hindi: true,
                cjk: true,
                arabic: true,
                emoji: true,
                arbitrary: true,
            },
            whitespace: WhitespaceFeatures {
                tab: true,
                nbsp: true,
                long_token: true,
                id_token: true,
            },
            inlines: InlineFeatures {
                emph: true,
                strong: true,
                strikethrough: true,
                underline: true,
                highlight: true,
                spoiler: true,
                subscript: true,
                superscript: true,
                code: true,
                math: true,
                link: true,
                autolink: true,
                wikilink: true,
                image: true,
                shortcode: true,
            },
            blocks: BlockFeatures {
                atx_heading: true,
                setext_heading: true,
                fenced_code: true,
                indented_code: true,
                thematic_break: true,
                html_block: true,
                table: true,
                math_block: true,
                block_quote: true,
                bullet_list: true,
                ordered_list: true,
                ordered_nonstandard_start: true,
                task_list: true,
                alert: true,
                nested_containers: true,
                multi_line_paragraph: true,
                wide_list_indent: true,
            },
            document: DocumentFeatures { front_matter: true, crlf: true, empty_corner_cases: true },
        }
    }

    /// [`Self::all`] minus tables: a table in a wide-indented item mis-tiles
    /// its row prefix (drag reveals on the leaked space), so tables stay
    /// top-tier-only until fixed. For layout invariants needing nesting.
    pub fn tier_a() -> Self {
        let all = Self::all();
        Self { blocks: BlockFeatures { table: false, ..all.blocks }, ..all }
    }

    /// [`Self::tier_a`] (so also no tables) minus the by-design-fuzzy axes:
    /// complex scripts and long unbreakable tokens (font-fallback / wrap
    /// divergence) and container nesting (inner columns shift under reveal).
    /// The layout-precision corpus; a failure here is a real bug.
    pub fn tier_b() -> Self {
        let a = Self::tier_a();
        Self {
            scripts: ScriptFeatures::default(),
            whitespace: WhitespaceFeatures { long_token: false, id_token: false, ..a.whitespace },
            blocks: BlockFeatures { nested_containers: false, ..a.blocks },
            ..a
        }
    }

    /// Container-block structure (lists, blockquotes, …) with nesting
    /// and tab/space indent variety, but plain ASCII paragraph content
    /// and no leaf blocks. The base for the structural presets below;
    /// invariants about indentation want the structure on most seeds
    /// without prose noise.
    fn containers_base() -> Self {
        let mut f = Self::default();
        f.whitespace.tab = true;
        f.blocks.nested_containers = true;
        f.blocks.multi_line_paragraph = true;
        f
    }

    /// Nested bullet / ordered / task lists, with marker, indent, and
    /// ordered-start variety.
    pub fn nested_lists() -> Self {
        let mut f = Self::containers_base();
        f.blocks.bullet_list = true;
        f.blocks.ordered_list = true;
        f.blocks.ordered_nonstandard_start = true;
        f.blocks.task_list = true;
        f
    }

    /// Nested blockquotes only. No alerts — they read as a distinct
    /// line kind, which would break the single-family homogeneity that
    /// the indent "kind preserved" invariants depend on.
    pub fn nested_blockquotes() -> Self {
        let mut f = Self::containers_base();
        f.blocks.block_quote = true;
        f
    }
}

pub(super) fn gen_word_f(src: &mut ByteSource, f: &Features) -> String {
    let s = &f.scripts;
    // Build weights for each kind, with 0 for disabled kinds. Sum must
    // be > 0 (ASCII is always available).
    let mut weights: Vec<u32> = vec![40];
    let mut kinds = vec![0u8]; // 0 = ascii
    if s.hindi {
        weights.push(4);
        kinds.push(1);
    }
    if s.cjk {
        weights.push(2);
        kinds.push(2);
    }
    if s.arabic {
        weights.push(2);
        kinds.push(3);
    }
    if s.emoji {
        weights.push(2);
        kinds.push(4);
    }
    if s.arbitrary {
        weights.push(1);
        kinds.push(5);
    }
    let pick = src.bias(&weights);
    match kinds[pick] {
        0 => ASCII_WORDS[src.draw(ASCII_WORDS.len())].to_string(),
        1 => HINDI_WORDS[src.draw(HINDI_WORDS.len())].to_string(),
        2 => CJK_WORDS[src.draw(CJK_WORDS.len())].to_string(),
        3 => ARABIC_WORDS[src.draw(ARABIC_WORDS.len())].to_string(),
        4 => EMOJI[src.draw(EMOJI.len())].to_string(),
        _ => random_codepoint(src),
    }
}

fn gen_separator_f(src: &mut ByteSource, f: &Features) -> &'static str {
    let w = &f.whitespace;
    let mut weights: Vec<u32> = vec![20];
    let mut kinds = vec![0u8];
    if w.tab {
        weights.push(1);
        kinds.push(1);
    }
    if w.nbsp {
        weights.push(1);
        kinds.push(2);
    }
    let pick = src.bias(&weights);
    match kinds[pick] {
        0 => " ",
        1 => "\t",
        _ => "\u{00A0}",
    }
}

pub(super) fn gen_text_f(src: &mut ByteSource, f: &Features) -> String {
    let n = 1 + src.bias(&[3, 4, 3, 2, 1]);
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            out.push_str(gen_separator_f(src, f));
        }
        out.push_str(&gen_word_f(src, f));
    }
    out
}

fn gen_inline_leaf_f(src: &mut ByteSource, f: &Features) -> String {
    let i = &f.inlines;
    let w = &f.whitespace;
    let mut weights: Vec<u32> = vec![20];
    let mut kinds: Vec<u8> = vec![0]; // 0 = text
    if i.code {
        weights.push(2);
        kinds.push(1);
    }
    if i.autolink {
        weights.push(1);
        kinds.push(2);
    }
    if i.wikilink {
        weights.push(1);
        kinds.push(3);
    }
    if i.shortcode {
        weights.push(1);
        kinds.push(4);
    }
    if i.image {
        weights.push(1);
        kinds.push(5);
    }
    if i.image && w.id_token {
        weights.push(1);
        kinds.push(6);
    }
    if w.long_token {
        weights.push(1);
        kinds.push(7);
    }
    let pick = src.bias(&weights);
    match kinds[pick] {
        0 => gen_text_f(src, f),
        1 => format!("`{}`", gen_code_text(src)),
        2 => "<https://x.test>".to_string(),
        3 => format!("[[{}]]", ASCII_WORDS[src.draw(ASCII_WORDS.len())]),
        4 => ":smile:".to_string(),
        5 => format!("![{}](https://x.test/i.png)", ASCII_WORDS[src.draw(ASCII_WORDS.len())]),
        6 => {
            let id = gen_identifier_token(src);
            format!("![{0}](imports/{0})", id)
        }
        _ => gen_long_token(src),
    }
}

fn gen_inline_element_f(src: &mut ByteSource, f: &Features, depth: usize) -> String {
    if depth == 0 {
        return gen_inline_leaf_f(src, f);
    }
    let i = &f.inlines;
    let mut weights: Vec<u32> = vec![12];
    let mut kinds: Vec<u8> = vec![0]; // 0 = leaf
    if i.emph {
        weights.push(2);
        kinds.push(1);
    }
    if i.emph {
        weights.push(2);
        kinds.push(2);
    }
    if i.strong {
        weights.push(2);
        kinds.push(3);
    }
    if i.strong {
        weights.push(1);
        kinds.push(4);
    }
    if i.emph && i.strong {
        weights.push(1);
        kinds.push(5);
    }
    if i.strikethrough {
        weights.push(2);
        kinds.push(6);
    }
    if i.highlight {
        weights.push(1);
        kinds.push(7);
    }
    if i.spoiler {
        weights.push(1);
        kinds.push(8);
    }
    if i.subscript {
        weights.push(1);
        kinds.push(9);
    }
    if i.superscript {
        weights.push(1);
        kinds.push(10);
    }
    if i.math {
        weights.push(1);
        kinds.push(11);
    }
    if i.link {
        weights.push(2);
        kinds.push(12);
    }
    let pick = src.bias(&weights);
    let kind = kinds[pick];
    if kind == 0 {
        return gen_inline_leaf_f(src, f);
    }
    let inner = gen_inline_tree_f(src, f, depth - 1);
    match kind {
        1 => format!("*{inner}*"),
        2 => format!("_{inner}_"),
        3 => format!("**{inner}**"),
        4 => format!("__{inner}__"),
        5 => format!("***{inner}***"),
        6 => format!("~~{inner}~~"),
        7 => format!("=={inner}=="),
        8 => format!("||{inner}||"),
        9 => format!("~{inner}~"),
        10 => format!("^{inner}^"),
        11 => format!("${inner}$"),
        _ => format!("[{inner}](https://x.test)"),
    }
}

fn gen_inline_tree_f(src: &mut ByteSource, f: &Features, depth: usize) -> String {
    let n = 1 + src.bias(&[3, 4, 3, 2, 1]);
    let elems: Vec<String> = (0..n)
        .map(|_| gen_inline_element_f(src, f, depth))
        .collect();
    let mut out = String::new();
    for (i, e) in elems.iter().enumerate() {
        if i > 0 && src.bias(&[8, 1]) == 0 {
            out.push(' ');
        }
        out.push_str(e);
    }
    out
}

fn gen_paragraph_f(src: &mut ByteSource, f: &Features) -> String {
    let lines = if f.blocks.multi_line_paragraph { 1 + src.bias(&[6, 2, 1]) } else { 1 };
    let mut s = String::new();
    for i in 0..lines {
        if i > 0 {
            s.push_str("  \n");
        }
        s.push_str(&gen_inline_tree_f(src, f, MAX_INLINE_DEPTH));
    }
    s.push('\n');
    s
}

fn gen_leaf_block_f(src: &mut ByteSource, f: &Features) -> String {
    let b = &f.blocks;
    let mut weights: Vec<u32> = vec![8];
    let mut kinds: Vec<u8> = vec![0]; // 0 = paragraph
    if b.atx_heading {
        weights.push(4);
        kinds.push(1);
    }
    if b.setext_heading {
        weights.push(1);
        kinds.push(2);
    }
    if b.fenced_code {
        weights.push(2);
        kinds.push(3);
    }
    if b.indented_code {
        weights.push(1);
        kinds.push(4);
    }
    if b.thematic_break {
        weights.push(1);
        kinds.push(5);
    }
    if b.table {
        weights.push(2);
        kinds.push(6);
    }
    if b.html_block {
        weights.push(2);
        kinds.push(7);
    }
    if b.math_block {
        weights.push(1);
        kinds.push(8);
    }
    let pick = src.bias(&weights);
    match kinds[pick] {
        0 => gen_paragraph_f(src, f),
        1 => {
            let level = 1 + src.draw(6);
            let hashes = "#".repeat(level);
            let inline = gen_inline_tree_f(src, f, MAX_INLINE_DEPTH);
            format!("{hashes} {inline}\n")
        }
        2 => {
            let underline = if src.bias(&[1, 1]) == 0 { "===" } else { "---" };
            format!("{}\n{underline}\n", gen_inline_tree_f(src, f, MAX_INLINE_DEPTH))
        }
        3 => gen_fenced_code(src),
        4 => gen_indented_code(src),
        5 => gen_thematic_break(src),
        6 => {
            // table with feature-aware inline contents per cell
            let n_cols = 2 + src.bias(&[2, 1]);
            let n_rows = 1 + src.bias(&[2, 1]);
            let alignments = [" --- ", " :--- ", " ---: ", " :---: "];
            let mut s = String::new();
            let row = |src: &mut ByteSource, f: &Features| {
                let cells: Vec<String> =
                    (0..n_cols).map(|_| gen_inline_tree_f(src, f, 1)).collect();
                format!("| {} |\n", cells.join(" | "))
            };
            s.push_str(&row(src, f));
            s.push('|');
            for _ in 0..n_cols {
                s.push_str(alignments[src.bias(&[3, 1, 1, 1])]);
                s.push('|');
            }
            s.push('\n');
            for _ in 0..n_rows {
                s.push_str(&row(src, f));
            }
            s
        }
        7 => {
            let tag = HTML_BLOCK_TAGS[src.draw(HTML_BLOCK_TAGS.len())];
            let inner = gen_inline_tree_f(src, f, 1);
            format!("<{tag}>{inner}</{tag}>\n")
        }
        _ => gen_math_block(src),
    }
}

fn gen_container_block_f(src: &mut ByteSource, f: &Features, depth: usize) -> Option<String> {
    let b = &f.blocks;
    let mut weights: Vec<u32> = Vec::new();
    let mut kinds: Vec<u8> = Vec::new();
    if b.block_quote {
        weights.push(5);
        kinds.push(0);
    }
    if b.bullet_list || b.ordered_list {
        weights.push(4);
        kinds.push(1);
    }
    if b.task_list {
        weights.push(2);
        kinds.push(2);
    }
    if b.alert {
        weights.push(1);
        kinds.push(3);
    }
    if weights.is_empty() {
        return None;
    }
    let pick = src.bias(&weights);
    let inner_depth = if f.blocks.nested_containers { depth } else { 0 };
    Some(match kinds[pick] {
        0 => {
            let inner = gen_block_seq_f(src, f, inner_depth);
            let prefix = if f.whitespace.tab && src.bias(&[2, 1]) == 1 { ">\t" } else { "> " };
            prefix_lines(&inner, prefix) + "\n"
        }
        1 => {
            let n = 1 + src.bias(&[2, 3, 2]);
            // Pick a style honoring the enabled list kinds.
            let style = if b.bullet_list && b.ordered_list {
                src.bias(&[6, 2, 2, 3, 1])
            } else if b.bullet_list {
                src.bias(&[3, 1, 1])
            } else {
                src.bias(&[1, 1]) + 3
            };
            // Ordered lists may begin at a non-1 / multi-digit number so
            // marker width varies within the list — the shape that
            // exposed marker-collapse bugs on indent.
            let start = if style >= 3 && b.ordered_nonstandard_start {
                match src.bias(&[3, 1, 1]) {
                    0 => 1,
                    1 => 2 + src.draw(4), // small non-1
                    _ => 9 + src.draw(2), // multi-digit by the 2nd item
                }
            } else {
                1
            };
            let mut out = String::new();
            for i in 0..n {
                let marker = match style {
                    0 => "- ".to_string(),
                    1 => "* ".to_string(),
                    2 => "+ ".to_string(),
                    3 => format!("{}. ", start + i),
                    _ => format!("{}) ", start + i),
                };
                // Continuation indent is the marker's content column;
                // `encode_indent` varies its spelling (spaces / tabs / mix).
                // `wide_list_indent` adds 0-3 cols so sub-lists sit past it
                // (relative indent 1-3) — still one list, not content.
                let extra = if f.blocks.wide_list_indent { src.bias(&[4, 2, 2, 1]) } else { 0 };
                let indent = encode_indent(marker.len() + extra, f, src);
                let inner = gen_block_seq_f(src, f, inner_depth);
                let body = indent_continuation(&inner, &indent);
                out.push_str(&format!("{marker}{body}\n"));
            }
            out
        }
        2 => {
            let n = 1 + src.draw(3);
            let mut out = String::new();
            for _ in 0..n {
                let mark = if src.bias(&[1, 1]) == 0 { ' ' } else { 'x' };
                let first_inline = gen_inline_tree_f(src, f, MAX_INLINE_DEPTH);
                if inner_depth > 0 && src.bias(&[5, 1]) == 1 {
                    let inner = gen_block_seq_f(src, f, inner_depth);
                    let body = indent_continuation(&inner, "  ");
                    out.push_str(&format!("- [{mark}] {first_inline}\n\n  {body}\n"));
                } else {
                    out.push_str(&format!("- [{mark}] {first_inline}\n"));
                }
            }
            out
        }
        _ => {
            let kind = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"][src.draw(5)];
            let inner = gen_block_seq_f(src, f, inner_depth);
            format!("> [!{kind}]\n{}\n", prefix_lines(&inner, "> "))
        }
    })
}

fn gen_block_f(src: &mut ByteSource, f: &Features, depth: usize) -> String {
    if depth == 0 || src.bias(&[7, 3]) == 0 {
        return gen_leaf_block_f(src, f);
    }
    gen_container_block_f(src, f, depth - 1).unwrap_or_else(|| gen_leaf_block_f(src, f))
}

fn gen_block_seq_f(src: &mut ByteSource, f: &Features, depth: usize) -> String {
    let n = 1 + src.bias(&[2, 3, 4, 4, 3, 2, 2, 1]);
    (0..n)
        .map(|_| gen_block_f(src, f, depth))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Feature-flagged doc generator. Default `Features` (all-false) produces
/// plain ASCII paragraphs; enabling flags opts into individual complexity
/// axes.
pub fn gen_doc(src: &mut ByteSource, features: &Features) -> String {
    let d = &features.document;
    if d.empty_corner_cases && src.bias(&[60, 1]) == 1 {
        return ["", " ", "\n", "\n\n", "  \n  \n", "\t", "\r\n"][src.draw(7)].to_string();
    }
    let mut out = String::new();
    if d.front_matter && src.bias(&[6, 1]) == 1 {
        out.push_str("---\ntitle: ");
        out.push_str(&gen_text_f(src, features));
        out.push_str("\n---\n\n");
    }
    let max_depth = if features.blocks.nested_containers { MAX_BLOCK_DEPTH } else { 0 };
    out.push_str(&gen_block_seq_f(src, features, max_depth));
    if d.crlf && src.bias(&[10, 1]) == 1 {
        out = out.replace('\n', "\r\n");
    }
    out
}

/// `gen_doc(Features::all())` repeated until the output reaches `target_bytes` (default
/// ~100 KB — Obsidian's stop-parsing threshold; if we render comfortably
/// at that size we cover the realistic upper end). Each repetition draws
/// from `src` so the doc is fully derived from the seed and shrinkable.
pub fn gen_doc_large(src: &mut ByteSource, target_bytes: usize) -> String {
    let mut out = String::new();
    while out.len() < target_bytes {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&gen_doc(src, &Features::all()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    /// Prints 20 example docs at varying byte counts. Run with:
    ///   cargo test -p workspace --lib markdown_doc_gen::tests::dump_examples -- --nocapture
    #[test]
    fn dump_examples() {
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let len = 16 + (seed as usize % 6) * 16;
            let mut buf = vec![0u8; len];
            rng.fill(&mut buf[..]);
            let mut src = ByteSource::new(&buf);
            let doc = gen_doc(&mut src, &Features::all());
            println!("--- seed {seed} ({len} bytes) ---");
            println!("{doc}");
        }
    }
}

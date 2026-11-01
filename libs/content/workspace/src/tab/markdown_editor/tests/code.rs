//! Code in fences and code files.

use super::harness::TestEditor;

#[test]
fn highlight_edge_inside_a_grapheme() {
    let mut ws = TestEditor::new("```python\nprint(1\u{fe0f}\u{20e3})\n```\n");
    ws.enter_frame();
}

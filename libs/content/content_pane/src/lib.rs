pub enum ContentPane {
    Markdown(lb_editor::Editor),
    Drawing,
}

impl ContentPane {
    pub fn new(title: &str, content: Vec<u8>) -> Self {
        if title.ends_with(".md") {
            Self::Markdown(lb_editor::Editor::new(&String::from_utf8(content).unwrap()))
        } else {
            Self::Drawing
        }
    }
}

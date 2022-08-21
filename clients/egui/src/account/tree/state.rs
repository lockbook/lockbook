use std::collections::HashSet;

use eframe::egui;

pub struct TreeState {
    pub id: egui::Id,
    pub max_node_width: f32,
    pub selected: HashSet<lb::Uuid>,
    pub renaming: NodeRenamingState,
    pub dnd: TreeDragAndDropState,
}

impl Default for TreeState {
    fn default() -> Self {
        Self {
            id: egui::Id::new("filetree"),
            max_node_width: 0.0,
            selected: HashSet::new(),
            dnd: TreeDragAndDropState::default(),
            renaming: NodeRenamingState::default(),
        }
    }
}

impl TreeState {
    pub fn toggle_selected(&mut self, id: lb::Uuid) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.dnd.is_primary_down && self.dnd.has_moved
    }

    pub fn dropped(&mut self, pos: Option<egui::Pos2>) {
        self.dnd.is_primary_down = false;
        self.dnd.has_moved = false;
        self.dnd.dropped = pos;
    }

    pub fn drag_caption(&self) -> String {
        let n = self.selected.len();
        format!("{} file{}", n, if n > 1 { "s" } else { "" })
    }
}

#[derive(Default)]
pub struct TreeDragAndDropState {
    pub is_primary_down: bool,
    pub has_moved: bool,
    pub dropped: Option<egui::Pos2>,
}

#[derive(Default)]
pub struct NodeRenamingState {
    pub id: Option<lb::Uuid>,
    pub tmp_name: String,
}

impl NodeRenamingState {
    pub fn new(f: &lb::File) -> Self {
        Self { id: Some(f.id), tmp_name: f.name.clone() }
    }
}

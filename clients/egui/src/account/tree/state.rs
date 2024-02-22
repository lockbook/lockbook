use std::{
    collections::HashSet,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

use eframe::egui;

#[derive(Debug)]
pub struct TreeState {
    pub id: egui::Id,
    pub max_node_width: f32,
    pub selected: HashSet<lb::Uuid>,
    pub expanded: HashSet<lb::Uuid>,
    pub renaming: NodeRenamingState,
    pub request_scroll: Option<lb::Uuid>,
    pub dnd: TreeDragAndDropState,
    pub update_tx: Sender<TreeUpdate>,
    pub update_rx: Receiver<TreeUpdate>,
}

impl Default for TreeState {
    fn default() -> Self {
        let (update_tx, update_rx) = mpsc::channel();
        Self {
            id: egui::Id::new("filetree"),
            max_node_width: 0.0,
            selected: HashSet::new(),
            expanded: HashSet::new(),
            dnd: TreeDragAndDropState::default(),
            renaming: NodeRenamingState::default(),
            request_scroll: None,
            update_tx,
            update_rx,
        }
    }
}

pub enum TreeUpdate {
    ExportFile((lb::File, PathBuf)),
}

impl TreeState {
    pub fn toggle_selected(&mut self, id: lb::Uuid) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
    }

    // returns true if the drag was released
    pub fn update_dnd(&mut self, i: &egui::InputState) -> bool {
        let mut released = false;

        self.dnd.is_primary_down = i.pointer.primary_down();

        // check events because sometimes pointer down and up will happen in the same frame
        // see clients/windows/src/input/pointer.rs
        for event in &i.events {
            if let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                ..
            } = event
            {
                if *pressed {
                    self.dnd.is_primary_down = true;
                    self.dnd.start_pos = *pos;
                } else {
                    released = true;
                }
            }
        }

        if self.dnd.is_primary_down {
            self.dnd.has_moved |= i.pointer.is_moving();
        }

        released
    }

    pub fn is_dragging_rect(&self, rect: egui::Rect) -> bool {
        rect.contains(self.dnd.start_pos) && self.is_dragging()
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

#[derive(Default, Debug)]
pub struct TreeDragAndDropState {
    pub is_primary_down: bool,
    pub start_pos: egui::Pos2,
    pub has_moved: bool,
    pub dropped: Option<egui::Pos2>,
}

#[derive(Default, Debug)]
pub struct NodeRenamingState {
    pub id: Option<lb::Uuid>,
    pub tmp_name: String,
}

impl NodeRenamingState {
    pub fn new(f: &lb::File) -> Self {
        Self { id: Some(f.id), tmp_name: f.name.clone() }
    }
}

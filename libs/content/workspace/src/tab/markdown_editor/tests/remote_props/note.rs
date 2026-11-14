//! The note open in a real workspace, as the app runs it: saves queue as
//! auto-save queues them, another device's version lands as a write to the
//! core, and the workspace's task manager launches loads and saves by its
//! own rules. What it launches is held, so a case runs each read or write,
//! and later records it, as steps of their own with any number between.

use std::sync::{Arc, Mutex, OnceLock};

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::FileType;
use web_time::{Duration, Instant};

use super::super::super::Editor;
use super::{Host, IMAGE};
use crate::task_manager::{Ran, Work};
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::ws::{TestWs, test_lb};

/// One account shared by every seed, with [`IMAGE`] 40 points square
/// beside the notes; each seed opens its own note in its own workspace.
fn account() -> Lb {
    static LB: OnceLock<Lb> = OnceLock::new();
    LB.get_or_init(|| {
        let lb = test_lb();
        let mut png = vec![];
        image::RgbaImage::new(40, 40)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let root = lb.get_root().unwrap().id;
        let file = lb.create_file(IMAGE, &root, FileType::Document).unwrap();
        lb.write_document(file.id, &png).unwrap();
        lb
    })
    .clone()
}

/// What frames applied to the note, in the workspace's order within each:
/// loads, then saves, then save launches.
pub(super) enum Landed {
    /// A load applied, with the text it read.
    Load(String),
    /// A save recorded as written, with its text.
    Saved(String),
    /// A save launched with the note's edits so far.
    Launched,
}

pub(super) struct Note {
    pub t: TestWs,
    pub id: Uuid,
    /// Launched work whose read or write hasn't run.
    held: Arc<Mutex<Vec<Work>>>,
    /// Work whose read or write ran, not yet recorded.
    ran: Vec<Ran>,
    /// Work recorded since the last frame, which applies it.
    recorded: Vec<Landed>,
    /// What frames applied since the last take.
    applied: Vec<Landed>,
}

impl Note {
    /// `md` open in its own tab, loaded and drawn.
    pub fn open(md: &str) -> Self {
        let mut t = TestWs::on(&account());
        t.ws.cfg.set_auto_save(false);
        let held = Arc::default();
        t.ws.tasks.held = Some(Arc::clone(&held));
        let id = t.doc(&format!("{}.md", Uuid::new_v4()), md);
        t.ws.open_file(id, true, true);
        let mut note = Self { t, id, held, ran: vec![], recorded: vec![], applied: vec![] };
        let start = Instant::now();
        loop {
            note.frame_after(0.0, vec![]);
            let mut src = ByteSource::new(&[]);
            while note.held() > 0 || note.ran() > 0 {
                while note.held() > 0 {
                    note.run(&mut src, &mut |_| {});
                }
                while note.ran() > 0 {
                    note.record_at(0, &mut |_| {});
                }
                note.frame_after(0.0, vec![]);
            }
            if note.shown().is_some_and(|md| md.settled()) {
                note.applied.clear();
                return note;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "the note never settled");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// The editor, while the note's tab is open and loaded.
    pub fn shown(&self) -> Option<&Editor> {
        self.t.ws.current_tab().and_then(|tab| tab.markdown())
    }

    /// The version on disk, which another device's edit starts from.
    pub fn disk(&self) -> String {
        let bytes = self.t.ws.core.read_document(self.id, false).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    /// What frames applied since the last take.
    pub fn take_applied(&mut self) -> Vec<Landed> {
        std::mem::take(&mut self.applied)
    }

    /// Something is recorded that the next frame applies.
    pub fn recorded(&self) -> bool {
        !self.recorded.is_empty()
    }

    /// A load is recorded and the next frame applies it.
    pub fn load_recorded(&self) -> bool {
        self.recorded.iter().any(|r| matches!(r, Landed::Load(_)))
    }

    /// Nothing is held, run, recorded or waiting in the task manager.
    pub fn idle(&self) -> bool {
        self.held() == 0
            && self.ran() == 0
            && self.recorded.is_empty()
            && !self.t.ws.tasks.busy(self.id)
    }

    /// The text of every tab of the note, closed ones the workspace still
    /// holds included.
    pub fn tab_texts(&self) -> Vec<String> {
        self.t
            .ws
            .tabs
            .iter()
            .filter(|(_, tab)| tab.id() == Some(self.id))
            .filter_map(|(_, tab)| tab.markdown())
            .map(|md| md.edit.renderer.buffer.current.text.clone())
            .collect()
    }

    /// Closes the note's tab, which queues its save.
    pub fn close(&mut self, log: &mut dyn FnMut(String)) {
        log("tab closes".into());
        self.t.ws.close_tab(0);
    }

    /// Opens the note in a new tab; its load launches with the next frame.
    pub fn reopen(&mut self, log: &mut dyn FnMut(String)) {
        log("tab reopens".into());
        self.t.ws.open_file(self.id, true, true);
    }

    /// The note as a fresh workspace opens it, or why it doesn't open in
    /// the editor.
    pub fn fresh(&self) -> Result<String, String> {
        let mut t = TestWs::on(&account());
        t.ws.open_file(self.id, true, false);
        t.idle(self.id);
        t.frame();
        let tab = t.ws.current_tab_mut().ok_or("no tab opens")?;
        match tab.markdown_mut() {
            Some(md) => Ok(md.edit.renderer.buffer.current.text.clone()),
            None => Err("the note doesn't open in the editor".into()),
        }
    }

    /// Runs and records everything in flight, in launch order, framing
    /// until nothing is left.
    pub fn settle(&mut self, log: &mut dyn FnMut(String)) {
        let mut src = ByteSource::new(&[]);
        for _ in 0..100 {
            if self.idle() {
                return;
            }
            while self.held() > 0 {
                self.run(&mut src, log);
            }
            while self.ran() > 0 {
                self.record_at(0, log);
            }
            self.frame_after(0.0, vec![]);
        }
        panic!("the note's loads and saves never settled");
    }

    /// Queues a save of every tab with edits, as auto-save does; it
    /// launches at the top of the next frame.
    pub fn save(&mut self, log: &mut dyn FnMut(String)) {
        log("save queued".into());
        self.t.ws.save_all_tabs();
    }

    /// Another device's version written to disk, as a sync writes it; the
    /// next frame queues its load.
    pub fn remote(&mut self, text: &str) {
        self.t
            .ws
            .core
            .write_document(self.id, text.as_bytes())
            .unwrap();
    }

    pub fn held(&self) -> usize {
        self.held.lock().unwrap().len()
    }

    pub fn ran(&self) -> usize {
        self.ran.len()
    }

    /// Whether the `i`th run work is a load.
    pub fn ran_load(&self, i: usize) -> bool {
        matches!(self.ran[i], Ran::Load(..))
    }

    /// Runs one launched read or write, drawn from those held.
    pub fn run(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) {
        let work = {
            let mut held = self.held.lock().unwrap();
            if held.is_empty() {
                return;
            }
            let i = src.draw(held.len());
            held.remove(i)
        };
        let ran = self.t.ws.tasks.run(work);
        log(match &ran {
            Ran::Load(_, Ok((_, bytes))) => {
                format!("load reads {:?}", String::from_utf8_lossy(bytes))
            }
            Ran::Load(_, Err(err)) => format!("load fails: {:?}", err.kind),
            Ran::Save { content, new_hmac_result: Ok(_), .. } => {
                format!("save writes {:?}", String::from_utf8_lossy(&content.bytes()))
            }
            Ran::Save { new_hmac_result: Err(err), .. } => format!("save fails: {:?}", err.kind),
        });
        self.ran.push(ran);
    }

    /// Records the `i`th run read or write for the next frame to apply.
    pub fn record_at(&mut self, i: usize, log: &mut dyn FnMut(String)) {
        let ran = self.ran.remove(i);
        match &ran {
            Ran::Load(_, Ok((_, bytes))) => {
                log("load recorded".into());
                self.recorded
                    .push(Landed::Load(String::from_utf8_lossy(bytes).into()));
            }
            Ran::Save { content, new_hmac_result: Ok(_), .. } => {
                log("save recorded".into());
                let text = String::from_utf8_lossy(&content.bytes()).into();
                self.recorded.push(Landed::Saved(text));
            }
            _ => log("failure recorded".into()),
        }
        self.t.ws.tasks.record(ran);
    }
}

impl Host for Note {
    fn editor(&self) -> &Editor {
        self.shown().expect("the note is open")
    }

    fn editor_mut(&mut self) -> &mut Editor {
        self.t
            .ws
            .current_tab_mut()
            .and_then(|tab| tab.markdown_mut())
            .expect("the note is open")
    }

    fn frame_after(&mut self, idle: f64, input: Vec<egui::Event>) {
        let held = self.held();
        if idle > 0.0 {
            self.t.frame_after(idle, input);
        } else {
            self.t.frame_with(input);
        }
        // the frame applied what was recorded, loads before saves, then launched
        let (loads, saves): (Vec<_>, Vec<_>) = std::mem::take(&mut self.recorded)
            .into_iter()
            .partition(|r| matches!(r, Landed::Load(_)));
        self.applied.extend(loads);
        self.applied.extend(saves);
        let launched = self.held.lock().unwrap()[held..]
            .iter()
            .filter(|w| matches!(w, Work::Save { .. }))
            .count();
        self.applied.extend((0..launched).map(|_| Landed::Launched));
    }
}

impl Drop for Note {
    fn drop(&mut self) {
        let _ = self.t.ws.core.delete_file(&self.id);
    }
}

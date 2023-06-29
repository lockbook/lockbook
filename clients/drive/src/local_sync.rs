use std::{fs::File, io::Read, path::PathBuf, sync::mpsc::channel};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::Drive;

impl Drive {
    fn sync(&self) {
        self.c
            .sync(Some(Box::new(|sp: lb::SyncProgress| {
                use lb::ClientWorkUnit::*;
                match sp.current_work_unit {
                    PullMetadata => println!("pulling file tree updates"),
                    PushMetadata => println!("pushing file tree updates"),
                    PullDocument(f) => println!("pulling: {}", f.name),
                    PushDocument(f) => println!("pushing: {}", f.name),
                };
            })))
            .unwrap();
    }

    pub fn check_for_changes(&self, mut dest: PathBuf) {
        self.c.get_account().unwrap();
        self.sync();

        self.c
            .export_file(self.c.get_root().unwrap().id, dest.clone(), false, None)
            .unwrap();
        File::open(&dest).unwrap().sync_all().unwrap();

        // Create a channel to receive file events
        let (tx, rx) = channel();

        dest.push(self.c.get_root().unwrap().name);
        dest = dest.canonicalize().unwrap();

        // Create a new watcher object
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();

        // Register the file for watching
        watcher.watch(&dest, RecursiveMode::Recursive).unwrap();

        println!("Watching for changes in {:?}", dest);
        for res in rx {
            println!("{:#?}", res);
            match res {
                Ok(event) => match event.kind {
                    EventKind::Any => {}
                    EventKind::Access(_) => {}
                    EventKind::Create(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let check = self.c.get_by_path(core_path.to_str().unwrap());
                        if check.is_err() {
                            self.c.create_at_path(core_path.to_str().unwrap()).unwrap();
                        } else {
                            println!("{:?}", event);
                        }
                    }
                    EventKind::Modify(_) => {
                        let mut f = File::open(event.paths[0].clone()).unwrap();
                        let mut buffer = vec![];
                        f.read_to_end(&mut buffer).unwrap();
                        println!("{:?}", &buffer[..]);
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let to_modify = self.c.get_by_path(core_path.to_str().unwrap()).unwrap();
                        self.c.write_document(to_modify.id, &buffer[..]).unwrap();
                    }
                    EventKind::Remove(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let to_delete = self.c.get_by_path(core_path.to_str().unwrap()).unwrap();
                        self.c.delete_file(to_delete.id).unwrap();
                    }
                    EventKind::Other => {}
                },
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    }
}
fn get_lockbook_path(event_path: PathBuf, dest: PathBuf) -> PathBuf {
    let mut ep_iter = event_path.iter();
    for _ in &dest {
        ep_iter.next();
    }
    PathBuf::from(&ep_iter)
}

#[test]
fn test() {
    let a = PathBuf::from("/a/b/c/d");
    let b = PathBuf::from("/a/b/c");
    let c = get_lockbook_path(a, b);
    assert_eq!(c, PathBuf::from("d"));
}

#[test]
fn test2() {
    let a = PathBuf::from(
        "/Users/siddhantsapra/Desktop/lockbook/lockbook/clients/drive/siddhant/test.md",
    );
    let b = PathBuf::from("/a/b/c");
    let c = get_lockbook_path(a, b);
    assert_eq!(c, PathBuf::from("d"));
}

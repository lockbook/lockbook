//! Windows doesn't call providers for content writes or new files, so watch the tree.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::*;

use crate::cf::{wide, Handle};

pub fn spawn(dir: PathBuf, tx: Sender<PathBuf>) {
    std::thread::spawn(move || {
        if let Err(err) = run(&dir, &tx) {
            eprintln!("watcher stopped: {err:?}");
        }
    });
}

fn run(dir: &Path, tx: &Sender<PathBuf>) -> windows::core::Result<()> {
    let w = wide(dir);
    let h = unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )?
    };
    let h = Handle(h);
    // DWORD-aligned, as ReadDirectoryChangesW requires.
    let mut buf = vec![0u32; 16 * 1024];
    loop {
        let mut len = 0u32;
        unsafe {
            ReadDirectoryChangesW(
                h.0,
                buf.as_mut_ptr() as _,
                (buf.len() * 4) as u32,
                true,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE
                    | FILE_NOTIFY_CHANGE_SIZE,
                Some(&mut len),
                None,
                None,
            )?
        };
        if len == 0 {
            // Overflow: rescan everything.
            let _ = tx.send(dir.to_path_buf());
            continue;
        }
        let base = buf.as_ptr() as *const u8;
        let mut offset = 0usize;
        loop {
            let info = unsafe { &*(base.add(offset) as *const FILE_NOTIFY_INFORMATION) };
            let name = unsafe {
                std::slice::from_raw_parts(info.FileName.as_ptr(), info.FileNameLength as usize / 2)
            };
            let _ = tx.send(dir.join(String::from_utf16_lossy(name)));
            if info.NextEntryOffset == 0 {
                break;
            }
            offset += info.NextEntryOffset as usize;
        }
    }
}

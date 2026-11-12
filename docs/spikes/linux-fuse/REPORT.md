# Linux FUSE mount: spike report (2026-11-10)

**Outcome.** `lb-fs fuse` mounts the desktop app's account at `~/Lockbook` through `fuser` and `fusermount3`. It needs no root and no libfuse. Three throwaway accounts on three VMs shared a folder and edited it from gedit, Kate and nano. Each edit reached the other machines in about 2.5 s. Three edits made within 0.7 s of each other merged identically everywhere. Saves based on stale content never overwrite: they become a `name (conflicted copy N).ext` beside the file. The weak spot is #2783: file managers don't refresh on remote changes.

Evidence: [collaboration-demo.mp4](collaboration-demo.mp4) (Ubuntu | Arch KDE | Arch sway, 2x speed), [share accepted](collaboration-share-accepted.png), [merged](collaboration-merged.png), plus per-step shots in this folder.

## What it took

- [libs/lb-fs/src/fuse.rs](../../../libs/lb-fs/src/fuse.rs), about 800 lines, plus a `fuse` subcommand in `lb-fs`. New dependencies: `fuser 0.18` (default features, so pure Rust) and `libc`.
- **Helper process, not inside the desktop app.** A FUSE server has to answer the kernel even when the GUI is busy or gone. A wedged GUI would otherwise hang every process that touches `~/Lockbook`. That mirrors the macOS File Provider extension. The helper opens the desktop app's own `~/.lockbook/egui` under lb-rs's multi-process locks, with no copy. During the demo the CLI and the helper ran against the same directory at once.
- **Opens are buffers.** `open` reads with `read_document_with_hmac`. `flush`/`fsync`/`release` write with `safe_write` against the base version. `ReReadRequired` turns the buffer into a conflicted copy.
- **The base is what the saving process read.** It's keyed by process (the thread's Tgid) and covers both save styles: Kate writes in place (`open`, `ftruncate`, `write`), while gedit and `sed -i` write a temp file and rename it over the original. The rename keeps the target's id, so shares and history follow the file. The mount requests `FUSE_ATOMIC_O_TRUNC`, and a handle-less `ftruncate` lands in the file's open writers, so a truncation is never a version of its own.
- **Sync → kernel cache.** On `MetadataChanged(Sync)`, `DocumentWritten(_, Sync)` and `IpcChangesApplied`, a thread diffs the directories and documents the kernel has seen. It then calls `inval_entry`/`inval_inode`. Opens don't keep the page cache, so reopening a file always shows synced content.
- lb-rs `background_work` does the syncing: 500 ms after local writes, and every 3 s while someone browses the mount (FUSE activity calls `app_foregrounded`).

Bugs this spike found and fixed in its own code, each reproduced on a VM first: a lock held across an `and_then` chain deadlocked `lookup`; Kate's handle-less `ftruncate` emptied the file; a stale in-place save overwrote a synced line; GIO saving on a worker thread defeated the per-process check.

## Per distro and desktop

| VM | Desktop | Result |
|---|---|---|
| Ubuntu 24.04, kernel 6.8 | GNOME 46, Nautilus 46.4, gedit | Mount appears in the Nautilus sidebar on its own, with eject. gedit saves through rename. Before saving, gedit warns "externally modified" because the mount reports lb-rs's `last_modified` as mtime. |
| Arch, kernel 7.2.14 | Plasma 6, Dolphin, Kate | Works, but no Places entry. Kate saves in place and leaves a `.kate-swp` swap file in the folder while editing. |
| Arch, kernel 7.2.14 | sway, foot, nano, `watch` | Works. `watch` polls, so it is the one view that updates live. |

## What doesn't work yet

1. **File watchers (#2783).** A remote create or edit raises no inotify event; I measured 25 s of `inotifywait` with nothing. Nautilus and Dolphin show remote changes only after F5 or re-entering the folder, and open editors don't notice. The kernel offers no notify path that reaches fsnotify. The only lever I know: the helper touches its own mount (`utimensat` produces `IN_ATTRIB`) for edits. Creates and deletes have no clean path. My understanding is that rclone mount has the same limitation.
2. **Editor temp files sync to collaborators** if they live past the 500 ms sync debounce: `.kate-swp`, `.goutputstream-*`, vim's `.swp`. This needs a local-only ignore list.
3. **One FUSE thread.** With lazy documents (#4240), an open can fetch from the network and stall the whole mount. That needs a multi-threaded session.
4. **Conflict rule edges.** An app that read v1 and then *appends* after a sync gets a needless conflicted copy. An app that never re-reads makes a new copy on every save until it reloads. macOS has the same shape: it rejects each save.
5. **Idle latency.** With nobody touching the mount, lb-rs polls every 5 min, so a remote edit can take that long to arrive. That's the desktop app's policy too.
6. **Not tested:** vim's default `backupcopy` (it renames the original away, which gives the file a new id); the snap/flatpak desktop app running next to the helper; large files; mount at a subfolder (#3960); out-of-space (#4336); two devices running git on the same repo at once.

Found by an independent code review afterwards (2026-11-11). These come from reading the code; none was reproduced on a VM:

7. **A huge truncate or write offset aborts the mount.** Buffers resize to whatever the kernel asks, so `truncate -s 100G` fails to allocate and every unsaved buffer is lost. Needs EFBIG above a cap.
8. **A rename over a file that is still open with unsaved writes** reads the source's last saved content, not its buffer. The later close writes to the deleted source. GLib renames before closing when the target was empty, so a gedit save into an empty file likely loses its text.
9. **vim's default save** (rename `x` to `x~`, create a new `x`, delete `x~`) gives the file a new id and skips the stale check.
10. **Any read-only open by the saving process resets its base**, so `backupcopy=yes` (read, then truncate and write) overwrites a synced change. The per-process versions are never pruned, so a reused pid inherits a stale base.
11. **A failed save at close drops the buffer.** That happens when the file was deleted remotely, at the data cap, or with two conflicted copies of one file in the same second (the name only has whole seconds).
12. **A rename that turns into a conflicted copy reports success.** For git's `index.lock` → `index`, git would believe the index moved.
13. **`mv a/x b/y` is a move then a rename.** It fails with EEXIST if `b/x` exists, and can leave the file in `b` under its old name.
14. Minor: `readdir` offsets index a freshly rebuilt list (deletes in between skip entries); `getattr` decrypts a whole document for its size, which downloads files that aren't local and fails offline; a handle-less truncate resizes other processes' buffers too.

**Git repos in the folder (2026-11-11, Ubuntu VM).** `git init`, commits, branches, `gc`, `fsck --full` and a `git clone` from GitHub all worked inside `~/Lockbook`. A second data directory imported the same account and synced. The exported repo there had all 7 commits and passed `git fsck --full`. Not tried: git on two devices at once. Lockbook would merge `.git/index` and refs like any other files, which is the known Dropbox-and-git hazard.

## Prior art: reused and rejected

- **`lb-fs` (NFS).** I reused the crate, the logger and the operation mapping: lookup by child name, rename as move plus rename, and rename-over writes into the target's id. I rejected the NFS server and `sudo mount.nfs`, the data-directory copy, read-modify-write of the whole document on every NFS write, `fill_cache` decrypting every document at mount (sizes are now cached per content version), `unwrap`s (lb errors map to errnos, the #4388 point), and the 30 s sync loop.
- **#1268 (`fuser`, `vfsrs`).** I used `fuser`. I skipped the cross-platform `vfsrs` layer: Windows Cloud Files is a placeholder-and-hydration model, not a VFS. What the platforms share is lb-rs's versioned read/write and the conflict rule, not an operation trait.
- **macOS File Provider (#5197, #5249).** I followed its versioned reads and writes and its shared data directory under lb-rs locks. One deviation: on `ReReadRequired`, macOS fails the save with a message. A FUSE call can't show UI, and many apps ignore `close()` errors, so Linux keeps the bytes as a conflicted copy (the Dropbox convention).
- **`origin/lb-bifs`** keeps a real folder in sync with base copies and a three-way merge. Its strengths: plain files, so inotify, offline use and every app just work, and no FUSE dependency in sandboxes. Its costs: a second copy of every file on disk, a watcher plus debounce that can read half-written files, and a merge done by the helper rather than lb-rs. It also has to detect renames from create/delete pairs, which loses ids, and with them share and history. FUSE keeps one source of truth and ids. Its price is #2783 and sandbox packaging. For collaboration on shared folders I'd keep FUSE and spend the watcher effort there.

## Effort to production (about 3–4 weeks)

- **Hardening, 1 week:** multi-threaded session, temp-file ignore list, vim/backup patterns, large files, tests over a mounted tree in CI (which needs `/dev/fuse`).
- **Watchers, 1 week, open-ended:** prototype the self-touch for edits, or accept F5 and document it.
- **Startup and sidebar, 3–4 days:** a desktop-app setting that starts the helper (systemd user unit or child process, sticky), unmount on quit, a GTK bookmark plus a KDE `user-places.xbel` entry.
- **Packaging, 3–5 days:** `.deb`, AUR and source builds just need `fuse3`. **Risk:** strict snap and flatpak can't mount FUSE without host help (snap's `fuse-support` interface is manually connected; flatpak needs a host-side helper). Unverified here.

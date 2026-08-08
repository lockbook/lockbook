# Multi-window edit: cursor jump and text mangling

Investigation notes (2026-07). Not a design proposal — a write-up of a QA bug and what the logs showed.

## Symptom

macOS: open the same note in two windows, type in one. Irregularly, the cursor jumps mid-typing and text can get mangled. Hard to repro consistently; usually within a few minutes of dual-window editing.

## Setup

Each window is its own workspace surface:

- its own markdown buffer
- its own **origin** UUID (tags writes so the writer can ignore its own `DocumentWritten`)
- shared lb document store in-process

Auto-save runs about once a second for dirty tabs.

## Two layers of failure

### 1. Idle surface stays dirty and keeps saving

“Dirty” is roughly `last_changed > last_saved`. Applying an **external** reload updates the buffer and can set `last_changed` via `text_updated`, even when the user never typed in that window.

So the idle second window can stay dirty and auto-save forever, republishing the document every tick.

### 2. Hmac and buffer content get out of phase (the corruption)

When a foreign write arrives, reload currently does roughly:

1. Set `md.hmac` to the new document version **immediately**
2. `buffer.reload(new_text)` — only **queues** merge ops
3. Ops apply later on the next editor `update()` (typically next input/frame)

Between (1) and (3):

```text
hmac  = version of text A
text  = still text B   (stale)
```

If auto-save clones content in that window, `safe_write` only checks that `old_hmac` matches the store. It does **not** check that the body is the content that produced that hmac. A stale body under a fresh hmac can **commit** and roll the document backward.

## What the logs showed

Instrumentation prefix: `[mw-edit]` (local only; not required to understand this write-up).

Two origins on one file; call them **T** (typing) and **O** (other):

1. **T** saves good content (e.g. length 14441) with a new hmac.
2. **O** is still dirty with older text (e.g. 14436), almost a full auto-save interval dirty.
3. Same frame on **O**:
   - reload starts: hmac ← T’s new hmac, merge ops queued, buffer still 14436
   - auto-save launches with **new hmac + old text**
4. That write **succeeds** and rolls disk back to 14436.
5. **T** gets `DocumentWritten` from **O**, reloads the rollback while focused and mid-edit:
   - selection side-effect: e.g. `(14442) → (14432)`
   - text length shrinks: e.g. `14442 → 14436`
   - `selection_user_moved=false` (OT shift from external replace, not a click)

Cursor jump + mangling are the user-visible side of absorbing a **stale overwrite**, not broken selection persistence.

## Scope

| Scenario | Idle “stuck dirty” republish | Hmac/content rollback while typing |
|----------|------------------------------|-------------------------------------|
| Two windows, same device, same note | Yes (this repro) | Yes |
| Separate devices | Unlikely unless that device is truly dirty | Possible on a device that is dirty when a sync lands (same deferred-apply race) |
| Two tabs, one shared buffer per file (today’s usual model) | No second saver | Same as single tab (sync / conflicts) |
| Two real surfaces for one file (two windows today; dual tabs if designed that way) | Yes | Yes |

Treat the unit of risk as an **editing surface** (origin + buffer + dirty), not “window” vs “tab.”

## Fix direction (not implemented here)

Both layers:

1. **Integrity:** never save when hmac and buffer disagree. Apply pending reload before clone/write, or only advance hmac when content is applied; refuse save with `pending_ops() > 0`.
2. **Dirty rule:** dirty only from **local** edits on that surface. Pure external reload must not leave the tab dirty / must not auto-save.

Either alone is incomplete: (2) alone still races when both surfaces edit; (1) alone still allows noisy reload ping-pong from an idle dirty surface.

Roughly foundry’s `edit` crate posture: pull/merge into the live buffer **before** CAS write; auto-sync after user edits, not after “facts about the world.”

## Key code (approx.)

- Reload + hmac: `workspace` completed-load path for markdown (`buffer.reload`, `md.hmac = …`)
- Dirty: tab `last_changed` when markdown reports `text_updated`
- Save: `task_manager` `clone_content` + `safe_write(…, old_hmac, …)`
- Skip own writes: `process_lb_updates` / `DocumentWritten` origin compare
- Selection shift on replace: `lb-rs` text `buffer` apply/reload OT

## Repro tips

1. Same note in two macOS windows.
2. Type steadily in one; leave the other open and idle.
3. Watch for cursor jump / truncated tail after ~1s auto-save cadence.
4. If instrumented: filter logs for `[mw-edit]`, especially `SEL-SIDE-EFFECT`, `ReReadRequired`, `will_reload=true` on the typing surface after the *other* origin saved, and saves with `content_len` behind the peer.

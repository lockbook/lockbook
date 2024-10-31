# Routine

Once the dog-fooding requirements of unreleased commits are met (see CONTRIBUTING.md for more info) the following QA routine must be completed on all platforms:

- Start a recording, stick it in a folder that can be referenced for analysis if a defect is found.
- Ensure that an upgrade of the new binary did not disrupt your experience (don't clean sync, use prior state).
- Go-to settings, ensure `debug-info` works and you see the expected QA version.
- Edit a document. Ensure updates are persisted.
- Draw on canvas. Ensure updates are persisted.
- Create a new document. Ensure updates are persisted.
- Create a new drawing. Ensure updates are persisted.
- See these changes reflected on some other device.
- Perform a clean sync.
- Create a new folder, create lots of docs.
- Sync them.
- Delete the folder.
- Sync it.

# Responsibilities

@ad-tra - Do the above routine on Android, and Linux.
@tvanderstad - Do the above routine on Windows and macOS.
@smailbarkouch - QA iPad and standby in-case anything goes wrong.
@parth - QA iPhone and release.

The release will take place when everyone's satisfied with QA (can happen asynchronously) and when Parth and Smail are both free for 90 minutes (should something go wrong).

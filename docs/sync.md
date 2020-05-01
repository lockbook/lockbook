### Initial Sync
To initially sync:
* Clients hit the server's `/get-updated-metadata` endpoint passing a `since_version` of 0, indicating that they want the metadata of all files (with any version since the beginning of time), and save the result.
* The metadata includes the `file_id` of each file; clients use this to fetch file contents from `FilesDB`. This can be done lazily or up-front to sync for offline editing.
* The `/get-updated-metadata` endpoint also returns an `update_version` whose value the client copies to `last_updated_version`. This should be done after files are synced because if it is done before and the client is terminated before files are synced, the client will think it has all updates through `last_updated_version`, when in fact some updates were not persisted.

### Subsequent Syncs
To sync after the initial sync:
* Clients hit the server's `/get-updated-metadata` endpoint passing a `since_version` of `last_updated_version`, indicating that they want the metadata of all files that have changed since the client last sucessfully checked for updates, and save the result.
* The metadata includes the `file_metadata_version` of each file. If this is different from the locally stored `file_metadata_version`, the file metadata has changed and will be overwritten.
* The metadata includes the `file_content_version` of each file. If this is different from the locally stored `file_content_version`, the file contents have changed and the new contents should be retrieved from `FilesDB`. If there are local changes, the user will be prompted to merge the different versions of the file (if it cannot be done automatically).

### Writing Changes
When a user makes a metadata-only change, the server responds with the metadata of the affected file, which the client saves. When the user makes a content change, the server still responds with the metadata of the affected file, but the client must also pass the `file_content_version` of the file. This version must match the server's current version in order for changes to be written. If the file has been updated by another client since this client last synced, the versions will not match, which the server will indicate. The client needs to sync, and because there are local changes to the file, the sync will prompt the user to merge the different versions of the file (if it cannot be done automatically).

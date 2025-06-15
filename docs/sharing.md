# Sharing

This technical design is in service of the [corresponding UX
design](sharing.md).

## Data Model

File metadata will include a share mode alongside each user access key. The
share mode can be `owner`, `update`, or `read`. Additionally, there will be a
new file type: a link. A link stores just the `id` of the file it links to.

## Server

Server will need to account for sharing in the following four endpoints:

1. `get_updates`
2. `get_document`
3. `update_content`
4. `upsert_metadata`

### Get Updates

`get_updates` will need to additonally return metadata for any files that have
any ancestors with user access keys for them.

### Get Document

`get_document` will need to check if the document has any ancestors with a user
access key for the user.

### Change Document Content

`update_content` will need to check if the document has any ancestors with a
user access key for the user that has share mode `owner` or `write`.

### Upsert File Metadata

`upsert_metadata` will need to check that user access keys on a file are not
modified in ways that violate the following requirements:

-   root files have exactly one owner user access key
-   non-root files do not have an owner user access key
-   owner user access keys of existing files are not modified
-   only the owner can update user access keys
-   each file can only have one user access key per user

## Lb-rs

Lb-rs will need a new repo to store link destinations.

The following functions should substitute links for their destinations (as able)
while recursing the file tree:

1. `get_children`
2. `get_and_get_children_recursively`
3. `get_file_by_path`
4. `list_paths`
5. `get_path_by_id`

Lb-rs will expose four new functions:

1. `share`
2. `get_pending_shares`
3. `set_link`
4. `delete_pending_share`

### Share

`share` is the function used to share a file. It accepts a file id, a username,
and a share mode, and it returns only a success or error. It fetches the public
key for the username and adds a user access key to the file for the user (the
file key encrypted with the user's public key). The share registers as an
unsynced file change and is only uploaded during the next sync.

The expected errors are:

-   `NoAccount`
-   `FileNonexistent`
-   `ClientUpdateRequired`
-   `CouldNotReachServer`
-   `UserNonexistent`
-   `FileAlreadySharedWithThatUser`

### Get Pending Shares

A pending share is a file which is shared with a user, but the user doesn't have
any links to the file. `get_pending_shares` returns the metadata for all such
files. It accepts no arguments.

The expected errors are:

-   `NoAccount`

### Set Link

Links are created using `create_file` using `FileType::Link`. `set_link` is the
function to set a link, similar to `write_document`. It accepts a file id for
the link and a file id for the destination. It returns only a success or error.

The expected errors are:

-   `NoAccount`
-   `FileNonexistent`
-   `LinkDestinationNonexistent`
-   `FileNotLink`

### Delete Pending Share

The only way for a file to cease to be shared with a user is for that user to
delete the share, which they do using `delete_share`. `delete_share` accepts a
file id and deletes the user access key for this user on that file (lb-rs must
reference the base version of the file if it needs to decrypt it). It does not
delete links to the file.

The expected errors are:

-   `NoAccount`
-   `FileNonexistent`
-   `FileNotShared`

## Clients

Sometimes, lb-rs will not be able to substitute links because the destination
does not exist locally. If a link destination does not exist, it's because the
destination file is not shared with the user. This can happen if a file is
shared with one user, then that user places a link to the file in a folder
shared with another user. In these situations, clients must render unsubstituted
link files (from their perspective, the only link files) by informing the user
that the file has not been shared with them and is therefore inaccessible. Note
that this requires an update to the data model in each client which corresponds
to the addition of the `Link` file type in lb-rs.

Clients should expose a context menu to share files as described in the sharing
UX doc, which uses lb-rs `share` function. Clients should check for pending
shares on start and after a sync using `get_pending_shares` and allow users to
accept shares by creating links (using `create_file` with
`file_type==FileType::Link`, then `set_link`) or decline them (using
`delete_pending_share`).

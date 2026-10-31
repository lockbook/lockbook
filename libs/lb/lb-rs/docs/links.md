# Links
A link is a file whose content is the id of another file. It has its own id, name, parent, owner, and key like any file; what it lacks is children or content of its own. Links exist for one job: placing a file someone shared with you in your own tree. A link to a file you own is invalid; so is a second link to the same target, a link inside a folder that is shared, and a link whose target you cannot see. Sync repairs each by deleting the link, unmoving its target, or removing the shares on a newly shared folder.

## Invisible
Listings never contain a link row; they substitute the target at the link's position under the link's name, and paths descend through links and ascend back through them. Only id-addressed calls — creating the link, fetching it by id — hand back the link itself. The sharer never sees the link's name or location; their file is untouched.

## The id contract
Operations addressed to a target you do not own act where it makes sense: rename, move, and delete act on your link; write, share, and duplicate act on the target. Creating inside a link's id fails — a link is not a folder — while creating inside the target's id, or at the link's path, works.

## When the target goes away
Target deleted: the link is deleted on the next sync. Share rejected: your link is deleted with your key, locally and at once. Share withdrawn from you: the target leaves your server tree, the client prunes it, the link is now broken and is deleted. Nothing is left behind to say what was there.

## One path
A file inside nested shares you have both accepted could have two paths, and has one. Resolving an id walks up from the file and leaves the share at the first ancestor that has a link to it, so the path through the inner link is the one reported and the only one that resolves; the path through the outer link appears once the inner link is deleted.

## Links to folders
A link may target a folder, and the target's children are the link's children at the link's path. A link to a link is representable in the type but unreachable in a valid tree: your own links are files you own, and no one else's can enter your tree.

## Rough edges
- The id contract is visible through the API — the same id behaves differently by operation — and no client should have to know it.
- One path at a time hides a file from the other place it could be; any canonical choice does (#4496).
- A deleted target or a lost share takes the link with it silently; nothing records what was there (#5141).

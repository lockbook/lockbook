# Sharing
A share is a key. Giving someone access to a file means encrypting that file's key to their public key and recording it on the file's metadata with a mode, read or write. Nothing moves: the file stays where it is, owned by whoever owned it. Access to a folder is access to everything under it, because the folder's key unlocks its children's keys. The server sees the share graph — who shared what with whom, at which mode — and uses it to decide what each account may pull and push; it never sees a key.

## Who may share
Granting anything at or below your own access is permitted: write may grant read or write, read may grant read. Sharing fetches the recipient's public key from the server each time; the share itself travels with the next sync like any metadata change. A recipient's mode is changed by sharing again with the new mode, which replaces their key.

## What the recipient sees
A shared subtree arrives as a detached root: a file with no parent the recipient can see, but with a key for them. It appears under *Shared with me*, grouped by sharer, readable and — with write — editable right there. There is no obligation to accept.

Accepting is placement. The recipient creates a [link](links.md) somewhere in their own tree pointing at the shared file; it then appears at the link's path under the link's name. The sharer never sees that name. *Pending* means "owned by someone else, shared with me, not deleted, and I have no live link to it," so deleting the link makes the share pending again.

Access is the maximum over a file and its ancestors. A write share inside a read share grants write to that subtree; rejecting one of two nested shares keeps whatever the other grants.

## Ending a share
The recipient declines by tombstoning their key, which also deletes their link. A rejection marks the key deleted rather than removing it, and the file's key is not rotated. Once the rejection syncs the file leaves the recipient's server tree and the client prunes it. The only owner-side ending is deleting the file.

## Ownership and billing
A file's owner is its parent's owner. Moving a file into someone else's shared folder makes it theirs, descendants included. The owner pays: usage counts only the files you own, and a recipient's write is charged against the sharer's cap, so a recipient can be refused for the sharer's quota. Downloads are counted per account but not billed.

## Constraints the tree imposes
A link may not sit inside a shared folder or be shared itself, and a folder containing a link cannot be shared. When a sync produces that state anyway, repair removes every share on the folder if the share is the new thing and deletes the link otherwise. Sharing a file that is already inside a shared folder is allowed and gives the recipient two roots.

## Nested shares
The outermost share determines tree separation: files in different shares are different trees for paths and document links. With links to both an outer and an inner shared folder, a file has one path at a time: the one that leaves the share at the nearest linked ancestor, walking up from the file. The other is neither listed nor resolvable ([links](links.md)).

## Rough edges
- `share_file` refuses write grants from anyone but the owner, while validation would accept them from any write-holder. Two layers, two rules.
- There is no owner-side revocation call, and no way to change a recipient's mode except re-sharing. Downgrading write to read is reported broken to an unknown extent (#4408).
- A former recipient keeps cryptographic access to later versions of anything they could once read ([keys](keys.md), #2165).
- Moving your own file into someone's write share transfers ownership and billing to them with no confirmation on any client.
- The shared-link repair tombstones *every* share on the newly shared folder, so a sync race can undo shares the owner just made, to every recipient.
- Nested shares: the two-roots view and the one-path-at-a-time rule are consequences, not decisions (#4496).
- `get_updates` returns every descendant of each changed shared file on every pull, regardless of version.
- Android defaults a new share to read; every other client defaults to write. An unknown recipient username surfaces as "You need an account to do that".

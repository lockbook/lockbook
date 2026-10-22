# Usage and limits
Usage is the sum over files you own of ciphertext size plus 1,000 bytes per file. Free is 25 MB; premium is 30 GB. Shared files count against their owner, so a collaborator's write into your folder is charged to you and refused if you are over cap, and moving your file into their folder moves the charge to them. Deleted files stop counting; their records persist unbilled.

## Enforcement
The server checks the owner's usage after staging a batch and rejects it if the result is over cap and not smaller than before. Downloads are counted per account and per server and not billed; per-account bandwidth limits engage only if the whole server passes 1 TB in a month.

## Sizes
Names up to 230 bytes, 254 encrypted. Usernames up to 32: lowercase letters, digits, `-_.@`. No request body limit; a 1 GiB document uploads as one body. Compression is skipped for tiny or binary-looking content.

## Rough edges
- "Not smaller than before" means an over-cap user can delete but cannot rename.

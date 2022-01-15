# Sharing

The sharing UX needs to balance simplicity with flexibility. Users need to be able to clearly tell who can access which documents and easily modify that access. They also need to be able to accomplish a wide variety of sharing-related tasks, including:

* sharing individual documents or whole folders
* sharing with readonly vs read/write access
* managing who is billed for shared documents
* linking to shared documents from shared documents and from unshared documents

We evaluate 3 design alternatives according to their suitability for individuals, teams, and enterprises. Individuals are users who store thoughts in lockbook and occassionally want to share a file with another user. Teams are small groups of users who generally want someplace to store personal things and someplace to store team things, which are shared with the whole team. Enterprises are large organizations that want to control how things are shared in sophisticated ways, such as having teams with subteams that have at least as much access. The designs we consider are:

1. Root sharing
2. Folder sharing
3. Policy Sharing

## 1. Root Sharing

### UX

Root sharing is similar to 1Password's vaults or Slack's workspaces. Under root sharing, users can create a new root and share that root with any set of users. Those users will have access to all files under that root. This means each set of sharees requires its own root. Paths for files under the new root begin with the root's name. The root's name is unique with usernames - no root name can be the same as any other root name or any username (similar to GitHub's rule that no org have be the same name as any other org or any user).

### Technical Implementation

Under this model, roots need only user access keys and non-root files need only folder access keys. This is a simplification of what we've already implemented.

### For Individuals

This model works poorly for individuals because they need to organize their documents according to what is shared with who. If I want to share my reading list with Parth, I need to create a vault shared with Parth and move my reading list into it. If I then want to share my notes on Malcom Gladwell's _Outliers_ with Parth and Raayan, I need to create a second vault shared with both of them and move my notes into it. Those notes would no longer live alongside all my other notes on books. A solution for this would be a symlink system, so that my notes can live in a shared vault but I can pretend they also live in a certain folder in my personal lockbook. The sharing workflow would create a vault for me and the sharee, move the file into it, then create a symlink for the file where it was originally. This would complicate linking logic because files no longer have a single unique path.

### For Teams

This model works well for teams because they can make a single team vault. As a member of the team, all my personal thoughts would live in my personal root whose path starts with my username and all my team thoughts would live in my team's root. I would be able to tell at a glance which files are shared with who and I would not be worried about accidentally sharing a personal thought with the team.

### For Enterprises

This model works okay for enterprises. In an enterprise, I would be part of multiple subteams e.g. my team, my department, and the whole organization. Because each set of sharees needs its own root, I would have one root for each team I am on. This makes it easy to share a file and know who it's shared with but difficult to organize across teams. For example if my team has technical documents that should be shared with an architecture council and cost estimation documents that should be shared with the budget team, we wouldn't be able to organize those in the same folder tree.

## 2. Folder Sharing

### UX

Folder sharing is similar to Google Doc's sharing model. All files live in the single root of some account. Users can share documents or folders (which share files recursively). Files shared with a user come up under a separate 'shared with me' section of the app's navigation - sharees can't organize files shared with them into folders with personal files. Sharees can move files only within the folders that they are shared through - an individually shared file cannot be moved by sharees at all. If a folder is shared with a user, that user can see all of the files in that folder recursively - there are no mechanisms to exclude access.

### Technical Implementation

Under this model, any file can have user access keys in addition to its folder access key, which is what we've implemented.

### For Individuals

This model works okay for individuals. As a sharer, I can easily share any file without moving it, but as a sharee, I cannot. Sharees have shared files appear alongside their other files. Again, a solution is symlinks. The sharee could organize symlinks to documents shared with them in whatever way they please.

### For Teams

This model works well for teams because they can make a team shared folder. The creator of the folder would 'own' it in some sense, as only they would be able to organize it alongside their other files, and other users would reference it using a path that starts with the owner's username. Some UI niceness would be required to make it easy for everyone to tell what is shared and with who.

### For Enterprises

This model works okay for enterprises because, like with the root sharing model, users who are members of multiple subteams would need to have one folder shared with them for each subteam and wouldn't be able to organize them. If we include a symlink feature, users would be able to organize the files shared with them into folders. Additionally, for each subteam, the superteam's common files could be symlinked into the subteam's shared folder _thereby making any files accessible to the superteam also accessible to the subteam_. The directory structure for the organization would be in a sense inverted, because when a parent team includes a child team, the shared folder for the child team would include the shared folder for the parent team. While workable, this creates a high burden of complexity considering enterprises still wouldn't be able to organize files in a way that mirror's the organization's structure. todo: restricted folders

## 3. Policy Sharing

### UX

Under policy sharing, users create policies which describe conditions under which a user has access to a file. Example policies include:

* Share document `d` with user `u`
* Share all documents with ancestor `a` with user `u` (folder sharing)
* Share all documents shared with user `u1` with user `u2` (subteam sharing)

More sophisticated options for constructing policies increase flexibility but also complexity.

### Technical Implementation

In a normal policy-based model, such as in Google's [Zanzibar](https://storage.googleapis.com/pub-tools-public-publication-data/pdf/41f08f03da59f5518802898f68730e247e23c331.pdf) access control system, a central server evaluates an individual access attempt using the relevant policies. To do this cryptographically is something I'm not even going to consider right now.

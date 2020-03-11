# Files, Folders, And Sharing
## User Experience
### Single-User Files
Users can create files and organize them into a folder hierarchy. Files can be created, deleted, moved, renamed, and edited and their state will be synchronized across devices. Files edited on multiple devices concurrently will prompt the user to merge the edits.

```
tvanderstad/
  - notes.md
  - blog_ideas.md
  - lockbook_project/
      - lockbook_design_doc.md
      - lockbook_todos.md
```

### Shared Files
Users can share files with other users. Files can be shared with `view`, `edit`, and `owner` level permission. `view` lets a user view the file. `edit` lets a user edit, rename, or delete a file. Owner lets a user share or unshare the file with other users including other owners.

```
pmehrotra/
  - journal.md
  - work_files/
      - lockbook_design_doc.md  <shared>

tvanderstad/
  - notes.md
  - blog_ideas.md
  - lockbook_project/
      - lockbook_design_doc.md  <shared>
      - lockbook_todos.md
```

Each file has a single name and file contents, but each user can store a shared file in their own location. Users with any level of permission to a file can move the file because it only moves the file for that user - the location of the file for other users is separate and not affected.

```
pmehrotra/
  - journal.md
  - lockbook_design_doc.md      <shared>
      

tvanderstad/
  - notes.md
  - blog_ideas.md
  - lockbook_project/
      - lockbook_design_doc.md  <shared>
      - lockbook_todos.md
```

### Shared Folders
Users can also share folders with other users. All the files and folders in a shared folder are shared with the same level of permission as the folder is shared with. The locations of files within a shared folder are the same for all users - only users with `edit` permission can move files and folders within a shared folder. All users can move the shared folder itself because the location of the folder for other users is separate and not affected.

```
pmehrotra/
  - journal.md
  - projects/
      - lockbook_project/  <shared>
          - lockbook_design_doc.md  <shared via lockbook_project/>
          - lockbook_todos.md       <shared via lockbook_project/>
      - other_project/
          - design.md
      

tvanderstad/
  - notes.md
  - blog_ideas.md
  - lockbook_project/      <shared>
      - lockbook_design_doc.md      <shared via lockbook_project/>
      - lockbook_todos.md           <shared via lockbook_project/>
```

Shared folders can contain shared files and folders. The inner shared item is shared at least as permissively as the containing folder is shared, but the inner shared item can also be shared with additional users or shared more permissively with the same users. If a file is shared with a user in two ways, for example view-shared via a folder and edit-shared directly, the more permissively sharing is applied (in this case edit). Putting an item in a shared folder can be thought of as sharing the item with the sharees of the folder - the item can still be shared with other users, both directly and by putting the item in other shared folders at the same time.

```
pmehrotra/
  - journal.md
  - projects/
      - lockbook_project/  <view-shared>
          - lockbook_design_doc.md  <view-shared via lockbook_project/>
          - lockbook_todos.md       <view-shared via lockbook_project/, but also edit-shared directly>
      - other_project/
          - design.md
      

tvanderstad/
  - notes.md
  - blog_ideas.md
  - lockbook_todos.md      <edit-shared directly>
  - lockbook_project/      <view-shared>
      - lockbook_design_doc.md      <view-shared via lockbook_project/>
      - lockbook_todos.md           <view-shared via lockbook_project/, but editable because also edit-shared directly>
```

### Restricted Files And Folders
TODO

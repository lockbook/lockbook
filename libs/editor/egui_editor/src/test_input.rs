pub static TEST_MARKDOWN: &str = TEST_MARKDOWN_54;

pub static TEST_MARKDOWN_ALL: [&str; 55] = [
    TEST_MARKDOWN_0,
    TEST_MARKDOWN_1,
    TEST_MARKDOWN_2,
    TEST_MARKDOWN_3,
    TEST_MARKDOWN_4,
    TEST_MARKDOWN_5,
    TEST_MARKDOWN_6,
    TEST_MARKDOWN_7,
    TEST_MARKDOWN_8,
    TEST_MARKDOWN_9,
    TEST_MARKDOWN_10,
    TEST_MARKDOWN_11,
    TEST_MARKDOWN_12,
    TEST_MARKDOWN_13,
    TEST_MARKDOWN_14,
    TEST_MARKDOWN_15,
    TEST_MARKDOWN_16,
    TEST_MARKDOWN_17,
    TEST_MARKDOWN_18,
    TEST_MARKDOWN_19,
    TEST_MARKDOWN_20,
    TEST_MARKDOWN_21,
    TEST_MARKDOWN_22,
    TEST_MARKDOWN_23,
    TEST_MARKDOWN_24,
    TEST_MARKDOWN_25,
    TEST_MARKDOWN_26,
    TEST_MARKDOWN_27,
    TEST_MARKDOWN_28,
    TEST_MARKDOWN_29,
    TEST_MARKDOWN_30,
    TEST_MARKDOWN_31,
    TEST_MARKDOWN_32,
    TEST_MARKDOWN_33,
    TEST_MARKDOWN_34,
    TEST_MARKDOWN_35,
    TEST_MARKDOWN_36,
    TEST_MARKDOWN_37,
    TEST_MARKDOWN_38,
    TEST_MARKDOWN_39,
    TEST_MARKDOWN_40,
    TEST_MARKDOWN_41,
    TEST_MARKDOWN_42,
    TEST_MARKDOWN_43,
    TEST_MARKDOWN_44,
    TEST_MARKDOWN_45,
    TEST_MARKDOWN_46,
    TEST_MARKDOWN_47,
    TEST_MARKDOWN_48,
    TEST_MARKDOWN_49,
    TEST_MARKDOWN_50,
    TEST_MARKDOWN_51,
    TEST_MARKDOWN_52,
    TEST_MARKDOWN_53,
    "1. *",
];

pub static TEST_MARKDOWN_0: &str = "# test";
pub static TEST_MARKDOWN_1: &str = "a";
pub static TEST_MARKDOWN_2: &str = "a\n";
pub static TEST_MARKDOWN_3: &str = "a\na";
pub static TEST_MARKDOWN_4: &str = "People think that a liar gains a victory over his victim. What I've learned is that a lie is an act of self-abdication, because one surrenders one's reality to the person to whom one lies, making that person one's master, condemning oneself from then on to faking the sort of reality that person's view requires to be faked...The man who lies to the world, is the world's slave from then on...There are no white lies, there is only the blackest of destruction, and a white lie is the blackest of all.";

pub static TEST_MARKDOWN_5: &str = "test";
pub static TEST_MARKDOWN_6: &str = "test\ntest";
pub static TEST_MARKDOWN_7: &str = "test\n\ntest";
pub static TEST_MARKDOWN_8: &str = "*a* b *c*";

pub static TEST_MARKDOWN_9: &str = "• test";
pub static TEST_MARKDOWN_10: &str = "😂";
pub static TEST_MARKDOWN_11: &str = "🦹🏿‍♀️";

pub static TEST_MARKDOWN_12: &str = "# a";
pub static TEST_MARKDOWN_13: &str = "# a\n";
pub static TEST_MARKDOWN_14: &str = "# a\n\n";
pub static TEST_MARKDOWN_15: &str = "# a\ntest";
pub static TEST_MARKDOWN_16: &str = "# a\n\ntest";
pub static TEST_MARKDOWN_17: &str = "# a *b*\n\n\n";
pub static TEST_MARKDOWN_18: &str = "+ # a\ntest";
pub static TEST_MARKDOWN_19: &str = "+";
pub static TEST_MARKDOWN_20: &str = "+ ";
pub static TEST_MARKDOWN_21: &str = "+ a";
pub static TEST_MARKDOWN_22: &str = "+ t\n\n";
pub static TEST_MARKDOWN_23: &str = "test\n+ test\n+ test2";
pub static TEST_MARKDOWN_24: &str = "+ test\n\t+ test2";
pub static TEST_MARKDOWN_25: &str = "+ a\n\t+ b\n\t+ c";
pub static TEST_MARKDOWN_26: &str = "+ t\n\t. test2\n  + test3";
pub static TEST_MARKDOWN_27: &str = "Test\n+ a very long line\n+ short\n+ another very long line";

pub static TEST_MARKDOWN_28: &str = "# bold\nTest *test* _test_ **test** __test__";

pub static TEST_MARKDOWN_29: &str = "```\nfn test() {\n}\n```";
pub static TEST_MARKDOWN_30: &str = "```\nfn test() {\n}\n```\n";
pub static TEST_MARKDOWN_31: &str = "```\nfn test() {\n}\n```\ntest";
pub static TEST_MARKDOWN_32: &str = "    fn a() {\n        test\n    }\ntest";

pub static TEST_MARKDOWN_33: &str = r#"# a
# Hello *World*

Goodbye world

```
fn test() {
}
```

The results of the _editor_ *editor* are kind of determining the near future plans for the gtk client,
 correct? In other words, if it goes well, we'll keep gtk and just `embed` the editor, if it goes â poorly, we might scrap gtk and prioritize egui, do I have that roughly right?


+ this is a test of an item with a soft break
inside it
+ [ ] test2
+ [x] test2
+ test2
    + more
    soft break
    + more
+ test
+ this is a random ~~codeblock~~ strikethrough
+ items should have some *character* and `code`
+ right?

> this is
> going to
> be ignored
> at the moment
"#;

pub static TEST_MARKDOWN_34: &str = r#"# Sharing
This technical design is in service of the [corresponding UX design](../design-ux/sharing.md).

## Data Model
File metadata will include a share mode alongside each user access key. The share mode can be `owner`, `update`, or `read`. Additionally, there will be a new file type: a link. A link stores just the `id` of the file it links to.

## Server
Server will need to account for sharing in the following four endpoints:
1. `get_updates`
2. `get_document`
3. `update_content`
4. `upsert_metadata`

### Get Updates
`get_updates` will need to additonally return metadata for any files that have any ancestors with user access keys for them.

### Get Document
`get_document` will need to check if the document has any ancestors with a user access key for the user.

### Change Document Content
`update_content` will need to check if the document has any ancestors with a user access key for the user that has share mode `owner` or `write`.

### Upsert File Metadata
`upsert_metadata` will need to check that user access keys on a file are not modified in ways that violate the following requirements:
* root files have exactly one owner user access key
* non-root files do not have an owner user access key
* owner user access keys of existing files are not modified
* only the owner can update user access keys
* each file can only have one user access key per user

## Core
Core will need a new repo to store link destinations.

The following functions should substitute links for their destinations (as able) while recursing the file tree:
1. `get_children`
2. `get_and_get_children_recursively`
3. `get_file_by_path`
4. `list_paths`
5. `get_path_by_id`

Core will expose four new functions:
1. `share`
2. `get_pending_shares`
3. `set_link`
4. `delete_pending_share`

### Share
`share` is the function used to share a file. It accepts a file id, a username, and a share mode, and it returns only a success or error. It fetches the public key for the username and adds a user access key to the file for the user (the file key encrypted with the user's public key). The share registers as an unsynced file change and is only uploaded during the next sync.

The expected errors are:
* `NoAccount`
* `FileNonexistent`
* `ClientUpdateRequired`
* `CouldNotReachServer`
* `UserNonexistent`
* `FileAlreadySharedWithThatUser`

### Get Pending Shares
A pending share is a file which is shared with a user, but the user doesn't have any links to the file. `get_pending_shares` returns the metadata for all such files. It accepts no arguments.

The expected errors are:
* `NoAccount`

### Set Link
Links are created using `create_file` using `FileType::Link`. `set_link` is the function to set a link, similar to `write_document`. It accepts a file id for the link and a file id for the destination. It returns only a success or error.

The expected errors are:
* `NoAccount`
* `FileNonexistent`
* `LinkDestinationNonexistent`
* `FileNotLink`

### Delete Pending Share
The only way for a file to cease to be shared with a user is for that user to delete the share, which they do using `delete_share`. `delete_share` accepts a file id and deletes the user access key for this user on that file (core must reference the base version of the file if it needs to decrypt it). It does not delete links to the file.

The expected errors are:
* `NoAccount`
* `FileNonexistent`
* `FileNotShared`

## Clients
Sometimes, core will not be able to substitute links because the destination does not exist locally. If a link destination does not exist, it's because the destination file is not shared with the user. This can happen if a file is shared with one user, then that user places a link to the file in a folder shared with another user. In these situations, clients must render unsubstituted link files (from their perspective, the only link files) by informing the user that the file has not been shared with them and is therefore inaccessible. Note that this requires an update to the data model in each client which corresponds to the addition of the `Link` file type in core.

Clients should expose a context menu to share files as described in the sharing UX doc, which uses core's `share` function. Clients should check for pending shares on start and after a sync using `get_pending_shares` and allow users to accept shares by creating links (using `create_file` with `file_type==FileType::Link`, then `set_link`) or decline them (using `delete_pending_share`)."#;

// todo: quotes can have nested headers and stuff
// todo: github supports this and it looks nice, prob want to replace > with |
pub static TEST_MARKDOWN_35: &str =
    r#"todo: https://raw.githubusercontent.com/mxstbr/markdown-test-file/master/TEST.md"#;

pub static TEST_MARKDOWN_36: &str = "* x\n\n* ";
pub static TEST_MARKDOWN_37: &str = "* *x*\n\n* ";
pub static TEST_MARKDOWN_38: &str =
    "* one\n* `two`\n    * three\n* `one`\n* two\n  * three and a half\n    * three";
pub static TEST_MARKDOWN_39: &str = "1. `one`\n2. `two`\n\t1. `three`\n\t2. `four`\n3. `five`\n4. `six`\n\n* `one`\n* `two`\n\t* `three`\n\t* `four`\n* `five`\n* `six`";
pub static TEST_MARKDOWN_40: &str = "* one\n\t* `two`";
pub static TEST_MARKDOWN_41: &str = "* item\n\nparagraph";
pub static TEST_MARKDOWN_42: &str = r#"# Numbered List Indentation
1. one
2. 


3. three
    1. four
        1. five"#;

pub static TEST_MARKDOWN_43: &str = "# Todo\n- [x] done\n- [ ] not done";
pub static TEST_MARKDOWN_44: &str = "# Todo\n- partially done\n\t- [x] done\n\t- [ ] not done";

pub static TEST_MARKDOWN_45: &str = r#"# Ambrose Burnside
![alt text](https://tile.loc.gov/storage-services/service/pnp/cwpbh/04900/04980r.jpg "title")
* qualities
    * good sideburns
    * poor commander of civil war forces

![alt text](https://invalidurl.com "title")
* this image has a broken link
"#;
pub static TEST_MARKDOWN_46: &str = "- ”";
pub static TEST_MARKDOWN_47: &str = "” x x";

pub static TEST_MARKDOWN_48: &str = "Visit our [website](http://lockbook.net)!";

pub static TEST_MARKDOWN_49: &str = r#"# Title
list
* __bold__ bulleted list item
  - [x] indented todo list item

```
code block
```

    code block

fin"#;
pub static TEST_MARKDOWN_50: &str = r#"# Editor Demo
## Featuring a **bold** subheading,
1. a **bold** list item,
    * an *italic and **bold italic*** list item,
    - [ ] a `code` list item,

```
a code block
```

> a quote,

a ~~rule~~
***
and a link to our [website](http://lockbook.net)!
"#;

pub static TEST_MARKDOWN_51: &str = r#"```
```"#;
pub static TEST_MARKDOWN_52: &str = r#"
If you try to create a checklist at the end of this file, the editor will crash.


_apples`apples`_

+ apples

- [ ] apples



- [ ] 
"#;
pub static TEST_MARKDOWN_53: &str = r#"# Rules
What do you know about rules?
***
Rules rule!
"#;

pub static TEST_MARKDOWN_54: &str = r#""#;

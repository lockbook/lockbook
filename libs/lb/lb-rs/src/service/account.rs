use crate::model::account::{Account, MAX_USERNAME_LENGTH};
use crate::model::api::{
    DeleteAccountRequest, GetPublicKeyRequest, GetUsernameRequest, NewAccountRequestV2,
};
use crate::model::errors::{LbErrKind, LbResult, core_err_unexpected};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::meta::Meta;
use crate::{DEFAULT_API_LOCATION, Lb};
use libsecp256k1::SecretKey;
use qrcode_generator::QrCodeEcc;

use crate::io::network::ApiError;

impl Lb {
    /// CoreError::AccountExists,
    /// CoreError::UsernameTaken,
    /// CoreError::UsernameInvalid,
    /// CoreError::ServerDisabled,
    /// CoreError::ServerUnreachable,
    /// CoreError::ClientUpdateRequired,
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        let username = String::from(username).to_lowercase();

        if username.len() > MAX_USERNAME_LENGTH {
            return Err(LbErrKind::UsernameInvalid.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        if db.account.get().is_some() {
            return Err(LbErrKind::AccountExists.into());
        }

        let account = Account::new(username.clone(), api_url.to_string());

        let root = Meta::create_root(&account)?.sign_with(&account)?;
        let root_id = *root.id();

        let last_synced = self
            .client
            .request(&account, NewAccountRequestV2::new(&account, &root))
            .await?
            .last_synced;

        db.account.insert(account.clone())?;
        db.base_metadata.insert(root_id, root)?;
        db.last_synced.insert(last_synced as i64)?;
        db.root.insert(root_id)?;
        db.pub_key_lookup
            .insert(Owner(account.public_key()), account.username.clone())?;

        self.keychain.cache_account(account.clone()).await?;

        tx.end();

        self.events.meta_changed();

        if welcome_doc {
            let welcome_doc = self
                .create_file("welcome.md", &root_id, FileType::Document)
                .await?;
            self.write_document(welcome_doc.id, Self::WELCOME_MESSAGE.as_bytes())
                .await?;
            self.sync(None).await?;
        }

        Ok(account)
    }

    #[instrument(level = "debug", skip(self, key), err(Debug))]
    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        if self.get_account().is_ok() {
            warn!("tried to import an account, but account exists already.");
            return Err(LbErrKind::AccountExists.into());
        }

        if let Ok(key) = base64::decode(key) {
            if let Ok(account) = bincode::deserialize(&key[..]) {
                return self.import_account_private_key_v1(account).await;
            } else if let Ok(key) = SecretKey::parse_slice(&key) {
                return self
                    .import_account_private_key_v2(key, api_url.unwrap_or(DEFAULT_API_LOCATION))
                    .await;
            }
        }

        let phrase: [&str; 24] = key
            .split([' ', ','])
            .filter(|maybe_word| !maybe_word.is_empty())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| LbErrKind::AccountStringCorrupted)?;

        self.import_account_phrase(phrase, api_url.unwrap_or(DEFAULT_API_LOCATION))
            .await
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        let server_public_key = self
            .client
            .request(&account, GetPublicKeyRequest { username: account.username.clone() })
            .await?
            .key;

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(LbErrKind::UsernamePublicKeyMismatch.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.insert(account.clone())?;
        self.keychain.cache_account(account.clone()).await?;

        Ok(account)
    }

    pub async fn import_account_private_key_v2(
        &self, private_key: SecretKey, api_url: &str,
    ) -> LbResult<Account> {
        let mut account =
            Account { username: "".to_string(), api_url: api_url.to_string(), private_key };
        let public_key = account.public_key();

        account.username = self
            .client
            .request(&account, GetUsernameRequest { key: public_key })
            .await?
            .username;

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.insert(account.clone())?;
        self.keychain.cache_account(account.clone()).await?;

        Ok(account)
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        let private_key = Account::phrase_to_private_key(phrase)?;
        self.import_account_private_key_v2(private_key, api_url)
            .await
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_account_private_key(&self) -> LbResult<String> {
        self.export_account_private_key_v1()
    }

    pub(crate) fn export_account_private_key_v1(&self) -> LbResult<String> {
        let account = self.get_account()?;
        let encoded: Vec<u8> = bincode::serialize(account).map_err(core_err_unexpected)?;
        Ok(base64::encode(encoded))
    }

    #[allow(dead_code)]
    pub(crate) fn export_account_private_key_v2(&self) -> LbResult<String> {
        let account = self.get_account()?;
        Ok(base64::encode(account.private_key.serialize()))
    }

    pub fn export_account_phrase(&self) -> LbResult<String> {
        let account = self.get_account()?;
        Ok(account.get_phrase()?.join(" "))
    }

    pub fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        let acct_secret = self.export_account_private_key_v1()?;
        qrcode_generator::to_png_to_vec(acct_secret, QrCodeEcc::Low, 1024)
            .map_err(|err| core_err_unexpected(err).into())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn delete_account(&self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, DeleteAccountRequest {})
            .await
            .map_err(|err| match err {
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        db.account.clear()?;
        db.last_synced.clear()?;
        db.base_metadata.clear()?;
        db.root.clear()?;
        db.local_metadata.clear()?;
        db.pub_key_lookup.clear()?;

        // todo: clear cache?

        Ok(())
    }

    const WELCOME_MESSAGE: &'static str = r#"# Markdown Syntax
Markdown is a language for easily formatting your documents. This document can help you get started.

## Styled Text
To style text, wrap your text in the corresponding characters.
| Style         | Syntax              | Example           |
|---------------|---------------------|-------------------|
| emphasis      | `*emphasis*`        | *emphasis*        |
| strong        | `**strong**`        | **strong**        |
| strikethrough | `~~strikethrough~~` | ~~strikethrough~~ |
| underline     | `__underline__`     | __underline__     |
| code          | ``code``            | `code`            |
| spoiler       | `||spoiler||`       | ||spoiler||       |
| superscript   | `^superscript^`     | ^superscript^     |
| subscript     | `~subscript~`       | ~subscript~       |

## Links
To make text into a link, wrap it with `[` `]`, add a link destination to the end , and wrap the destination with `(` `)`. The link destination can be a web URL or a relative path to another Lockbook file.
```md
[Lockbook's website](https://lockbook.net)
```
> [Lockbook's website](https://lockbook.net)

## Images
To embed an image, add a `!` to the beginning of the link syntax.
```md
![Lockbook's favicon](https://lockbook.net/favicon/favicon-96x96.png)
```
> ![Lockbook's favicon](https://lockbook.net/favicon/favicon-96x96.png)

## Headings
To create a heading, add up to six `#`'s plus a space before your text. More `#`'s create a smaller heading.
```md
# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6
```
> # Heading 1
> ## Heading 2
> ### Heading 3
> #### Heading 4
> ##### Heading 5
> ###### Heading 6

## Lists
Create a list item by adding `- `, `+ `, or `* ` for a bulleted list, `1. ` for a numbered list, or `- [ ] `, `+ [ ] `, or `* [ ] ` for a task list at the start of the line. The added characters are called the *list marker*.
```md
* bulleted list item
- bulleted list item
+ bulleted list item

1. numbered list item
1. numbered list item
1. numbered list item

- [ ] task list item
- [x] task list item
```
>* bulleted list item
>- bulleted list item
>+ bulleted list item
>
>1. numbered list item
>1. numbered list item
>1. numbered list item
>
>- [ ] task list item
>- [x] task list item

List items can be nested. To nest an inner item in an outer one, the inner item's line must start with at least one space for each character in the outer item's list marker: usually 2 for bulleted lists, 3 for numbered lists, or 2 for tasks lists (the trailing `[ ] ` is excluded).
```md
* This is a bulleted list
    * An inner item needs at least 2 spaces
1. This is a numbered list
    1. An inner item needs at least 3 spaces
* [ ] This is a task list
    * [ ] An inner item needs at least 2 spaces
```
> * This is a bulleted list
>   * An inner item needs at least 2 spaces
> 1. This is a numbered list
>    1. An inner item needs at least 3 spaces
> * [ ] This is a task list
>   * [ ] An inner item needs at least 2 spaces

List items can contain formatted content. For non-text content, each line must start with the same number of spaces as an inner list item would.
```md
* This item contains text,
    > a quote
    ### and a heading.
* This item contains two lines of text.
The second line doesn't need spaces.
```
> * This item contains text,
>   > a quote
>   ### and a heading.
> * This item contains two lines of text.
> The second line doesn't need spaces.

## Quotes
To create a block quote, add `> ` to each line.
```md
> This is a quote
```
> This is a quote

Like list items, block quotes can contain formatted content.
```md
> This quote contains some text,
> ```rust
> // some code
> fn main() { println!("Hello, world!"); }
> ```
> ### and a heading.

> This quote contains two lines of text.
The second line doesn't need added characters.
```
> This quote contains some text,
> ```rust
> // some code
> fn main() { println!("Hello, world!"); }
> ```
> ### and a heading.

> This quote contains two lines of text.
The second line doesn't need added characters.

## Alerts
To create an alert, add one of 5 tags to the first line of a quote: `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, or `[!CAUTION]`. An alternate title can be added after the tag.
```md
> [!NOTE]
> This is a note.

> [!TIP]
> This is a tip.

> [!IMPORTANT]
> This is important.

> [!WARNING]
> This is a warning.

> [!CAUTION] Caution!!!!!
> This is a caution.
```
> [!NOTE]
> This is a note.

> [!TIP]
> This is a tip.

> [!IMPORTANT]
> This is important.

> [!WARNING]
> This is a warning.

> [!CAUTION] Caution!!!!!
> This is a caution.

## Tables
A table is written with `|`'s between columns and a row after the header row whose cell's contents are `-`'s.
```md
| Style         | Syntax              | Example           |
|---------------|---------------------|-------------------|
| emphasis      | `*emphasis*`        | *emphasis*        |
| strong        | `**strong**`        | **strong**        |
```
> | Style         | Syntax              | Example           |
> |---------------|---------------------|-------------------|
> | emphasis      | `*emphasis*`        | *emphasis*        |
> | strong        | `**strong**`        | **strong**        |

## Code
A code block is wrapped by two lines containing three backticks. A language can be added after the opening backticks.
```md
    ```rust
    // some code
    fn main() { println!("Hello, world!"); }
    ```
```
> ```rust
> // some code
> fn main() { println!("Hello, world!"); }
> ```

You can also create a code block by indenting each line with four spaces. Indented code blocks cannot have a language.
```md
    // some code
    fn main() { println!("Hello, world!"); }
```
>     // some code
>     fn main() { println!("Hello, world!"); }

## Thematic Breaks
A thematic break is written with `***`, `---`, or `___` and shows a horizontal line across the page.
```md
***
---
___
```
> ***
> ---
> ___
"#;
}

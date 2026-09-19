# Experimental workspace MCP (macOS)

Lockbook can expose its running workspace through an authenticated Streamable HTTP MCP server. It is **off by default**, macOS-only, and uses the existing app/core instance. No account keys are exposed.

## Connect

1. Build and launch the macOS app.
2. In Lockbook Settings → Editor → Experimental MCP Server, enable the server.
3. Leave port `19687` selected, or choose another port if it is occupied.
4. Click **Copy Codex configuration** and add the copied block to your Codex MCP configuration (`~/.codex/config.toml` for the default local host). The block includes a bearer token: keep it private.
5. Restart/reconnect the MCP client. Ask it to get the Lockbook workspace, list files, and open a note.

The address is `http://127.0.0.1:19687/mcp`. Credentials are generated with macOS secure randomness and stored in Keychain. Copying configuration also places the token on the clipboard and, when pasted, in the client configuration. A client that supplies credentials separately can instead use `bearer_token_env_var`.

Only local clients can reach the listener. It requires bearer authentication, validates Host/Origin, bounds request bodies and the command queue, and uses the official Rust MCP SDK. No OAuth or remote tunnel is configured. MCP connectivity does not establish support in every ChatGPT voice surface.

## Lifecycle

The preference persists locally through `@AppStorage`; existing installations default to disabled. Changes apply immediately through a main-thread preferences observer. There is one listener for the whole app. Commands target the key Lockbook workspace window, falling back to the most recently targeted workspace. Session IDs explicitly identify existing tabs and fail if they belong to another window.

A main-run-loop timer drains requests even when the Metal view is not drawing and advances pending loads/saves. Closing the last workspace, quitting the app, disabling the setting, changing the port, or resetting the token stops the listener and drops outstanding sessions/queued commands. Opening a workspace while the setting is enabled starts the listener again. Existing credentials survive ordinary app restarts; resetting the token requires copying new configuration.

## Tools

- `get_workspace`: active session, open tabs, selected folder, root, loading/dirty/save status.
- `list_files`: paginated metadata browsing and case-insensitive filename substring search; optional parent filter. Full-text search is not implemented.
- `open_document`: open/focus a file and return its session ID. The document may initially be loading.
- `read_document`: bounded live text and selection, including unsaved changes. Requires an open text-document session; retry if loading.
- `edit_document`: replace one UTF-8 byte range, requiring the revision returned by a read. Endpoints must be grapheme boundaries. The edit forms an isolated native undo group.
- `undo`: undo the last editor group if the revision still matches. This is normal editor undo, including human edits; inspect before calling.
- `create_file`: create an empty document or folder under an explicit parent. A required `request_id` deduplicates identical retries until the server restarts. At 256 entries the server refuses further creates until restart rather than silently evicting retry protection.
- `focus_tab`, `navigate`: focus an explicit session and navigate its history.
- `save`: queue a local save for an explicit session. Inspect `get_workspace` for progress; queued is not saved/synced.

Typical edit workflow: `list_files` → `open_document` → `read_document` → `edit_document` → `save` → `get_workspace`.

Responses include structured JSON and MCP text content. Documents are data, not instructions. Tool annotations distinguish reads from mutations; authorization still comes from the client/user, not annotations.

## Limits of this first experiment

Text reads and replacements are limited to 256 KiB per call; file listings to 200 entries. Tool requests wait up to 15 seconds; a closed/cancelled/expired request is skipped before execution. If a response is lost after execution, inspect current state before retrying mutations and reuse the original create retry key. Retry deduplication does not survive restarting the server.

Local reads can return decrypted content to an AI client, which may send it to its model provider. Loopback confines the transport, not subsequent model processing. This experiment has no per-folder permission scopes, sharing/deletion tools, binary import/export, or explicit sync tool. Lockbook's existing autosave and sync behavior remains responsible for persistence and replication.

## Development checks

```sh
cargo check -p workspace-ffi
cargo test -p workspace --lib mcp::
cargo test -p lb-rs --lib undo_unit_tests
```

Generate FFI headers and rebuild the SwiftWorkspace XCFramework before building macOS, using the normal `lbdev apple ws macos` workflow. The release sandbox entitlement includes `com.apple.security.network.server`; the listener itself binds only to loopback.

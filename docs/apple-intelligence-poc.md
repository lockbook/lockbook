# Apple Intelligence preview

Implemented on `model-poc`; no fm-rs dependency, API key, local server, or model download UI.

## Try it

On an Apple silicon Mac with macOS 26+, Apple Intelligence enabled and its model ready, and Xcode 26+ selected:

```sh
./utils/dev/run-apple-ai-poc.sh
```

Quit the installed Lockbook app first. The launcher builds the desktop preview and reuses the installed Mac app's account when `~/.lockbook` does not exist. Set `LOCKBOOK_PATH` to override this. It does not copy or migrate the database. Ordinary Lockbook sync still applies to saved chats and settings.

Create a chat and select **Apple Intelligence** from the provider chooser (or add it from the existing model menu). It has one model, **Apple Intelligence**, with internal ID `on-device`. Ask a question, then a follow-up. Stop cancels the native request; the existing retry and saved-history flows are retained.

Use the chat's **⋯ → Add note…** menu to copy a saved Markdown/text note into the draft, inspect or edit it, and send. Notes above 12 KB are rejected; paste a smaller excerpt instead. The model can also call `list`, `read_note`, `request_access`, and `edit_note` through the existing Rust dispatcher. Access requests use the normal approval card; reading and editing obey the chat’s granted scope. Add note remains an optional shortcut.

A UI-independent smoke test:

```sh
cargo run -p lb-apple-ai --example chat -- 'Say hello to Lockbook in one sentence.'
```

## Implementation

`libs/apple-ai` builds a small Swift static library and owns the Rust interface. Rust sends bounded JSON containing instructions and conversation history. Swift constructs a Foundation Models session and streams text through a C callback. Swift has no direct network client or file access; its tool callbacks are routed to the Rust harness. Inference uses `SystemLanguageModel.default` on device; there is no cloud fallback.

Callback buffers are borrowed only for the call and copied immediately. Request IDs route callbacks to Rust channels; Swift never holds a Rust object pointer. Dropping a request removes its route before cancelling the Swift task, so late callbacks cannot reach another turn. Native sessions are recreated from saved history per user turn and remain alive across tool calls. Swift emits a tool-call event and suspends on a continuation; Rust runs its existing scope checks, approval UI, and dispatcher, then returns the matching result to Swift. Results are matched by request and call IDs. Duplicate or late results are ignored. Cancelling a session drains pending continuations. Previous turns’ tool results become context text when restoring history.

The four existing flat tool schemas are converted to native `GenerationSchema` properties, supporting required/optional string and boolean arguments. Unsupported property types fail explicitly. Tools use Apple’s real `Tool` protocol and `GeneratedContent` arguments, rather than parsing tool requests out of prose.

The chat adapter caps output at 768 tokens and requests at 128 KB, and times out after 120 seconds of waiting for the next completion/tool call. Time spent at the Rust permission card does not count against this timeout. On SDK/runtime 26.4+, native token counting checks the context budget before generation. Earlier supported versions rely on the framework's context error. Refusals and availability failures surface in chat. Token usage is not measured in this preview.

Builds without a supported Mac/SDK use an unavailable stub. FoundationModels is weak-linked and the bridge targets macOS 14 so older systems can still launch the application. The working inference path is macOS Apple silicon; iOS inference and native app packaging remain follow-up work. The shared workspace FFI library builds, but the installed Swift app has not been replaced.

The lockfile also advances `lb-fonts` to `5f27cc81283b46b77283054cb5cf51b324fd5de5`, whose Phosphor font export is required by the existing workspace source; the previous pin failed to compile before reaching this feature.

## Verification

Verified locally with Xcode 26.5/macOS 26.6.2:

- Desktop and workspace FFI builds succeed.
- All 66 chat unit tests pass.
- Bridge lifecycle tests pass with native inference enabled and with `LB_APPLE_AI_DISABLE=1`.
- Real on-device tests cover streaming, restored history, tool request/result/answer, cancellation while awaiting a tool result, duplicate results, and oversized-context errors.
- Native adapter tests exercise all four advertised schemas and resume with synthetic note contents. A native harness test checks that denying access produces the existing denied tool row without granting scope.
- Desktop binary inspection confirms weak FoundationModels linkage and the Swift runtime rpath.

```sh
cargo test -p lb-apple-ai
cargo test -p lb-apple-ai --test native -- --ignored --test-threads=1
DYLD_LIBRARY_PATH=/usr/lib/swift cargo test -p workspace tab::chat --lib
DYLD_LIBRARY_PATH=/usr/lib/swift cargo test -p workspace native_ --lib -- --ignored --test-threads=1
LB_APPLE_AI_DISABLE=1 cargo test -p lb-apple-ai --lib
cargo build -p lockbook-desktop -p workspace-ffi
```

Native tests use synthetic text only. Visual interaction checks could not be completed because the computer-use Accessibility/Screen Recording permissions were pending; the note-picker interaction still needs manual review.

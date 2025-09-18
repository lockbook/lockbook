# CLI
Lockbook's CLI is built around a sophisticated tab completion behavior. Install the cli using your favorite [package manager](installing.md) or reach out to us if yours isn't listed there. You can configure our CLI to open your favorite text editor allowing you to rapidly jump to your desired note and edit it quickly.

See `lockbook completions` for configuring completions manually. Use the `LOCKBOOK_EDITOR` environment variable to choose a [supported editor](https://github.com/lockbook/lockbook/blob/master/clients/cli/src/edit.rs#L64-L73).

`lockbook export` can copy files from within your lockbook to your filesystem, making it easy to create decrypted backups.

See [extending](extending.md) for more CLI highlights.
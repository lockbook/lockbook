# lb-rs & server

lb-rs & server just require a stable rust toolchain to build.

lb-rs can be pointed to a server via the `API_URL` env var, by default clients are engineered to connect to our production server: `https://api.prod.lockbook.net`.

Tests are configured to connect to `https://localhost:8000`.

You can run the server locally by executing `lbdev server` for local dev.
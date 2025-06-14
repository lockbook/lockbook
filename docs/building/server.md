# lb-rs & server

lb-rs & server just require a stable rust toolchain to build.

lb-rs can be pointed to a server via the `API_URL` env var, by default clients are engineered to connect to our production server: `https://api.prod.lockbook.net`.

tests are configured to connect to `https://localhost:8000`.

you can run the server locally by executing `./run-server.sh` in `utils/dev`. This will source a set of environment variables intended for local development.

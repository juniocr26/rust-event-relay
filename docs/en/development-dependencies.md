[Português brasileiro](../pt-BR/development-dependencies.md) | [README](../../README.md)

# Development dependencies and recovery

`Cargo.toml` declares the application metadata, dependency version ranges and enabled features. `Cargo.lock` records exact resolved versions and checksums, including transitive dependencies. Commit the lockfile for this application; `--locked` prevents silent resolution changes. Intentional dependency updates use `cargo update`, followed by checks and lockfile review.

`cargo fetch --locked` downloads dependencies without building. `cargo build --locked` compiles the application and dependencies, downloading missing sources when necessary. The first fetch needs network access; a populated cache can support offline commands with `--offline`.

`CARGO_HOME` contains registry indexes, downloaded crate archives, extracted sources and git checkouts when used. It may also contain Cargo configuration or credentials; do not commit it or store secrets in tracked files. Compiled output does not belong there. `target/` contains compiled dependencies, binaries and incremental state; it does not replace the dependency source cache.

Docker uses `CARGO_HOME=/app/.cargo-cache` and `CARGO_TARGET_DIR=/app/target`. The entire repository is mounted at `/app`, so both directories are physically visible on the host. No named volumes are used. The entrypoint creates absent directories as the non-root developer user. On Linux, first align UID/GID; see [Docker](docker-and-configuration.md).

If only `.cargo-cache/` is deleted, run `docker compose exec app cargo fetch --locked`; if only `target/` is deleted, run `docker compose exec app cargo build --locked`. Stop active Cargo/application processes before deleting their directories. A clean rebuild from the repository root is:

```bash
docker compose down
rm -rf .cargo-cache target
docker compose up -d
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo test --locked
```

The deletion discards only ignored downloads/build artifacts, not source or `Cargo.lock`. `up` recreates missing directories; fetch restores dependency sources; build recreates artifacts. If the image is also absent or the Dockerfile/UID/GID changed, use `docker compose up -d --build`. Neither `down` nor rebuilding the image removes the host directories.

Native Cargo normally uses `~/.cargo` for dependencies and repository `target/` for builds. To use the same explicit paths natively: `CARGO_HOME="$PWD/.cargo-cache" CARGO_TARGET_DIR="$PWD/target" cargo fetch --locked`. Native and container artifacts are platform-specific; run `cargo clean` when switching incompatible targets/toolchains. A valid lockfile alone is insufficient for offline recovery after all source caches are deleted.

PostgreSQL follows the same visible-state philosophy through `.dockerized-postgres/`, but it holds database contents rather than regenerable Cargo artifacts. The recovery above leaves database contents intact. `docker compose down` preserves all three directories. See [database storage and deliberate reset](docker-and-configuration.md).

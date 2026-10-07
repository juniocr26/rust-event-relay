FROM rust:1.95.0-bookworm
# Database migrations are development tooling, not application dependencies.
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install sqlx-cli --version 0.8.6 --locked \
      --no-default-features --features rustls,postgres --root /opt/sqlx \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git
COPY --chmod=755 docker/sqlx.py /usr/local/bin/sqlx
ARG LOCAL_UID=1000
ARG LOCAL_GID=1000
RUN groupadd --gid "$LOCAL_GID" developer \
    && useradd --uid "$LOCAL_UID" --gid "$LOCAL_GID" --create-home --shell /bin/bash developer \
    && rustup component add rustfmt clippy
WORKDIR /app
ENV CARGO_HOME=/app/.cargo-cache CARGO_TARGET_DIR=/app/target
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/dev-entrypoint
USER developer
ENTRYPOINT ["dev-entrypoint"]
CMD ["sleep", "infinity"]

FROM rust:1.95.0-bookworm
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

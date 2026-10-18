FROM node:22.22.0-bookworm-slim AS node
FROM rust:1.95.0-bookworm
# SQLx CLI provides development migrations; the application uses SQLx separately.
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 supervisor python3-psycopg2 postgresql-client ca-certificates libnss3 libnspr4 libdbus-1-3 libatk1.0-0 libatk-bridge2.0-0 libcups2 libdrm2 libxkbcommon0 libatspi2.0-0 libx11-6 libxcomposite1 libxdamage1 libxext6 libxfixes3 libxrandr2 libgbm1 libxcb1 libpango-1.0-0 libcairo2 libasound2 fonts-liberation \
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
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s /usr/local/lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s /usr/local/lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
COPY --chmod=755 docker/tests /usr/local/bin/tests
WORKDIR /app/backend
ENV CARGO_HOME=/app/.cargo-cache CARGO_TARGET_DIR=/app/target
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/dev-entrypoint
COPY docker/supervisor/normal.conf /etc/supervisor/supervisord.conf
COPY --chmod=755 docker/supervisor/supervisor /usr/local/bin/supervisor
COPY --chmod=755 docker/supervisor/relay-http /usr/local/bin/relay-http
USER developer
ENTRYPOINT ["dev-entrypoint"]
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/supervisord.conf"]
COPY --chmod=755 docker/supervisor/relay-worker /usr/local/bin/relay-worker
COPY --chmod=755 docker/supervisor/relay-consumer /usr/local/bin/relay-consumer

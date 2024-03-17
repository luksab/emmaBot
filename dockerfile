# start with rust image
ARG APP_NAME=lukas-bot
FROM rust:slim-bullseye AS build
WORKDIR /app

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=bind,source=.sqlx,target=.sqlx \
    <<EOF
set -e
cargo build --locked --release
cp ./target/release/lukas-bot /bin/app
EOF

FROM debian:bullseye-slim AS final
WORKDIR /app

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser

# Make sure the permissions are correct
RUN chown -R appuser:appuser /app

USER appuser

# Copy the executable from the "build" stage.
COPY --from=build /bin/app /bin/

CMD ["/bin/app"]

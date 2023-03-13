# start with rust image
FROM rust:1.64.0
WORKDIR /app
# copy the source code
COPY src src
COPY Cargo.toml Cargo.toml
COPY schema.sql schema.sql
# build the project
RUN cargo build --release
# copy the binary to the image
RUN cp target/release/lukas-bot /usr/local/bin

# Modified from https://github.com/lukemathwalker/cargo-chef#running-the-binary-in-alpine

FROM clux/muslrust:stable AS chef
USER root
RUN cargo install --locked cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --profile server --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN cargo build --profile server --target x86_64-unknown-linux-musl --bin server

FROM alpine AS runtime
WORKDIR /app
COPY --from=builder /app/server/assets ./assets
RUN chown -R nobody:nobody /app/assets
COPY --from=builder /app/target/x86_64-unknown-linux-musl/server/server /usr/local/bin/
USER nobody
EXPOSE 8080
CMD ["/usr/local/bin/server"]

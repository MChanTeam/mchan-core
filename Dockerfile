FROM rust:1-alpine AS build
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
	&& printf 'fn main() {}\n' > src/main.rs \
	&& cargo build --locked --release \
	&& cargo clean --release --package MChan \
	&& rm src/main.rs

COPY src ./src
COPY templates ./templates
COPY migrations ./migrations
COPY docs/PRIVACY.md docs/RULES.md docs/CHANGELOG.md ./docs/
RUN cargo build --locked --release

FROM alpine:3.22
RUN adduser -D -H mchan \
	&&mkdir /data \
	&& chown mchan:mchan /data
WORKDIR /app
COPY --from=build /app/target/release/MChan /usr/local/bin/mchan
COPY static ./static
USER mchan
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/mchan"]

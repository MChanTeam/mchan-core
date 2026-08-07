FROM rust:1-alpine AS build
WORKDIR /app
COPY . .
RUN cargo build --locked --release

FROM alpine:3.22
RUN adduser -D -H mchan \
	&&mkdir /data \
	&& chown mchan:mchan /data
WORKDIR /app
COPY --from=build /app/target/release/MChan /usr/local/bin/mchan
COPY --from=build /app/static ./static
USER mchan
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/mchan"]

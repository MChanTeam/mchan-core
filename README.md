# MChan

## Requirements

- Docker
- Rust and Cargo for local development

## Start Docker at boot

On OpenRC, enable and start the Docker service:

```sh
sudo rc-update add docker default
sudo rc-service docker start
```

## Run with Docker

Build the image:

```sh
docker build -t mchan .
```

Start the container:

```sh
docker run --rm --name mchan -p 3000:3000 mchan
```

Open <http://localhost:3000>.

The container listens on port `3000`.

Stop the container with `Ctrl+C`.

Build the image again after a code change:

```sh
docker build -t mchan .
```

## Run locally

Run the application with Cargo:

```sh
cargo run
```

Open <http://localhost:3000>.

## Docker files

- `Dockerfile` uses Alpine Linux for the build and runtime images.
- `.dockerignore` removes local files from the build context.
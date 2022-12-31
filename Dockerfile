# Image containing dev dependencies
FROM rust:1.66.0-slim-bullseye AS tools
WORKDIR /build

# Remove the default, let the repo override choose its own
RUN rustup toolchain remove 1.66.0

RUN apt-get update -y && \
  apt-get install -y \
    cmake \
    jq \
  && \
  rm -rf /var/lib/apt/lists/*

COPY rust-toolchain.toml ./

# Force rustup to install override toolchain
RUN rustc --version

# Workspace skeleton image for cache coherence
FROM tools AS skel

COPY scripts/install-skeleton.sh scripts/
COPY . ../repo

RUN scripts/install-skeleton.sh ../repo .

# Fat build image
FROM tools AS build

# Load skeleton into a clean layer for caching against the manifests only
COPY --from=skel /build/Cargo.lock /build/Cargo.toml ./
COPY --from=skel /build/crates crates

# Perform an initial fetch and build of the dependencies
RUN cargo fetch --locked
RUN cargo build --locked --profile=docker

# Load source code - source-only changes will begin building at this point
COPY scripts/install-skeleton.sh scripts/
COPY crates crates

# Skeleton has newer mtime than repo, touch all entrypoints to force rebuild
RUN scripts/install-skeleton.sh -t .

# Now perform a full build of the workspace
RUN cargo build --locked --profile=docker

# Base image for any output containers, to save space with common layers
FROM debian:bullseye-slim AS base
WORKDIR /opt/the-q

RUN apt-get update -y && \
  apt-get install -y --no-install-recommends \
    && \
    rm -rf /var/lib/apt/lists/*

COPY .env .env.prod ./

# Core bot image
FROM base AS bot
COPY --from=build /build/target/docker/the-q bin/
CMD ["bin/the-q"]

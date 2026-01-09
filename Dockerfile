# Image containing dev dependencies
FROM rust:slim-trixie AS tools
WORKDIR /build

# Remove the default, let the repo override choose its own
RUN rustup toolchain uninstall $(rustup toolchain list)

RUN apt-get update -y && \
  apt-get install -y \
    cmake \
    git \
    jq \
    libclang-dev \
    libmagickcore-7.q16hdri-dev \
    pkgconf \
    protobuf-compiler \
  && \
  rm -rf /var/lib/apt/lists/*

COPY rust-toolchain.toml ./

# Force rustup to install override toolchain
RUN rustc --version

# Skeleton image of the "best" initial commit
FROM tools AS skel-init

COPY .git .git/
COPY Cargo.lock ./
COPY scripts/checkout-init.sh scripts/install-skeleton.sh scripts/

RUN scripts/checkout-init.sh ../repo && scripts/install-skeleton.sh ../repo .

# Workspace skeleton image for cache coherence
FROM tools AS skel-fini

COPY scripts/install-skeleton.sh scripts/
COPY . ../repo

RUN scripts/install-skeleton.sh ../repo .

# Fat build image
FROM tools AS build

# Load INIT skeleton into a clean layer for caching against the manifests only
COPY --from=skel-init /build/Cargo.lock /build/Cargo.toml ./
COPY --from=skel-init /build/crates crates

# Perform an initial fetch and build of the INIT dependencies
RUN cargo fetch --locked
RUN cargo build --locked --profile=docker --bin the-q

# Load FINAL skeleton
COPY --from=skel-fini /build/Cargo.lock /build/Cargo.toml ./
COPY --from=skel-fini /build/crates crates

# Rebuild dependencies with current lockfile
RUN cargo fetch --locked
RUN cargo build --locked --profile=docker --bin the-q

# Load source code - source-only changes will begin building at this point
COPY scripts/install-skeleton.sh scripts/
COPY crates crates

# Skeleton has newer mtime than repo, touch all entrypoints to force rebuild
RUN scripts/install-skeleton.sh -t .

# Now perform a full build of the workspace
RUN cargo build --locked --profile=docker --bin the-q

# Base image for any output containers, to save space with common layers
FROM debian:trixie-slim AS base
WORKDIR /opt/the-q

RUN apt-get update -y && \
  apt-get install -y --no-install-recommends \
    ffmpeg \
    graphviz \
    libmagickcore-7.q16hdri \
  && \
  rm -rf /var/lib/apt/lists/*

COPY .env .env.prod ./

# Core bot image
FROM base AS bot
COPY --from=build /build/target/docker/the-q bin/
CMD ["bin/the-q"]

#!/bin/sh

cd "$(dirname "$0")"
set -xe

COMPOSE=docker-compose
which "$COMPOSE" || COMPOSE=podman-compose

"$COMPOSE" --env-file .env.compose "$@"

#!/bin/sh

[ -f .env.compose ] && . ./.env.compose
export GF_USER
export GF_PASSWD

COMPOSE=docker-compose
which "$COMPOSE" || COMPOSE=podman-compose

"$COMPOSE" "$@"

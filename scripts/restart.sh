#!/bin/bash

export NAPCAT_UID=$(id -u)
export NAPCAT_GID=$(id -g)
export BOT_MESSAGE_PREFIX="[Bot]:"

echo "NAPCAT_UID=$NAPCAT_UID"
echo "NAPCAT_GID=$NAPCAT_GID"
echo "BOT_MESSAGE_PREFIX=$BOT_MESSAGE_PREFIX"

docker compose down --remove-orphans

docker compose up -d

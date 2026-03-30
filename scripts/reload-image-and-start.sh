#!/bin/bash

export NAPCAT_UID=$(id -u)
export NAPCAT_GID=$(id -g)
export BOT_MESSAGE_PREFIX="[Bot]:"

echo "NAPCAT_UID=$NAPCAT_UID"
echo "NAPCAT_GID=$NAPCAT_GID"
echo "BOT_MESSAGE_PREFIX=$BOT_MESSAGE_PREFIX"

docker compose down --remove-orphans

IMAGE_NAME=qq-bot
IMAGE_TAG=0.0.1

# rm docker containers and images if exists
CONTAINERS=$(docker ps -a -q -f name=$IMAGE_NAME)
if [ -n "$CONTAINERS" ]; then
    docker rm -f $CONTAINERS
fi

IMAGES=$(docker images -q $IMAGE_NAME)
if [ -n "$IMAGES" ]; then
    docker rmi -f $IMAGES
fi

docker load -i qq-bot-0.0.1.tar.gz

docker compose up -d

#!/bin/env bash

IMAGE_NAME=qq-bot
IMAGE_TAG=0.0.1

rm -f qq-bot-0.0.1.tar.gz

echo "Will build image: $IMAGE_NAME:$IMAGE_TAG"

# rm docker containers and images if exists
CONTAINERS=$(docker ps -a -q -f name=$IMAGE_NAME)
if [ -n "$CONTAINERS" ]; then
    docker rm -f $CONTAINERS
fi
IMAGES=$(docker images -q $IMAGE_NAME)
if [ -n "$IMAGES" ]; then
    docker rmi -f $IMAGES
fi

docker build --progress=plain --no-cache --platform=linux/amd64 -t qq-bot:0.0.1 -f ./Dockerfile ./

docker save qq-bot:0.0.1 | gzip >qq-bot-0.0.1.tar.gz

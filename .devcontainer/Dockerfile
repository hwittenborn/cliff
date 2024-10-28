FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

ARG deps="\
    libgtk-4-dev \
    libdbus-1-dev \
    pkg-config \
    libadwaita-1-dev \
    golang \
    libclang-dev \
    "
RUN apt-get update \
    && apt-get install -y ${deps} \
    && rm -rf /var/lib/apt/lists/* \
    && rm -rf /var/lib/apt/lists/*

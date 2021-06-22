# Benchmark Dockerfile for tealdeer
#
# To run the benchmarks, execute
#
#     docker build -t tldr-benchmark .
#     docker run --privileged --rm -it tldr-benchmark
#
# as root in the directory of this Dockerfile. This will build the compared
# clients and benchmark them with `hyperfine` at the end.
#
# The `--privileged` flag is needed to drop the disk caches before every run. If
# you want to test with hot caches or don't want to use this flag, you will have
# to remove the `--prepare` line from the `hyperfine` command at the end of this
# file and rebuild the image.

################################################################################

FROM rust AS tealdeer-builder

WORKDIR /build
RUN git clone https://github.com/dbrgn/tealdeer.git \
        && cd tealdeer \
        && cargo build --release \
        && mkdir /build-outputs \
        && cp target/release/tldr /build-outputs/tealdeer

################################################################################

FROM ubuntu:latest AS tldr-c-builder

WORKDIR /build
RUN apt-get update && apt-get install -y build-essential git && rm -rf /var/lib/apt/lists/*
RUN git clone https://github.com/tldr-pages/tldr-c-client.git \
        && cd tldr-c-client \
        && DEBIAN_FRONTEND=noninteractive ./deps.sh \
        && make \
        && mkdir /build-outputs /deps \
        && cp tldr /build-outputs/tldr-c \
        && cp deps.sh /deps/tldr-c-deps.sh

################################################################################

FROM haskell:8.6 AS haskell-builder

WORKDIR /build

RUN git clone https://github.com/psibi/tldr-hs.git \
        && cd tldr-hs \
        && stack build

RUN git clone https://github.com/gutjuri/fast-tldr \
        && cd fast-tldr \
        && stack build

RUN mkdir /build-outputs \
        && find tldr-hs/.stack-work/dist -type f -iname tldr -exec mv '{}' /build-outputs/tldr-hs \; \
        && find fast-tldr/.stack-work/dist -type f -iname tldr -exec mv '{}' /build-outputs/fast-tldr \;

################################################################################

FROM node:slim AS node-builder

WORKDIR /build-outputs
RUN npm install tldr \
        && cp $(which node) . \
        && echo './node -- ./node_modules/.bin/tldr "$@"' > tldr-node \
        && chmod +x tldr-node

################################################################################

FROM euantorano/zig:0.8.0 AS zig-builder

WORKDIR /build
RUN apk add git \
        && git clone https://gitlab.com/ve-nt/outfieldr.git \
        && cd outfieldr \
        && git submodule init \
        && git submodule update \
        && zig build -Drelease-safe \
        && mkdir /build-outputs \
        && cp bin/tldr /build-outputs/outfieldr

################################################################################

FROM ubuntu:latest AS benchmark

ENV LANG="en_US.UTF-8"

WORKDIR /deps
RUN apt-get update && apt-get install -y wget unzip python3 python3-venv && rm -rf /var/lib/apt/lists/*
COPY --from=tldr-c-builder /deps/* ./
RUN for file in *; do DEBIAN_FRONTEND=noninteractive sh $file; done

WORKDIR /clients
COPY --from=tealdeer-builder /build-outputs/* ./
COPY --from=tldr-c-builder /build-outputs/* ./
COPY --from=haskell-builder /build-outputs/* ./
RUN wget -qO tldr-bash https://4e4.win/tldr && chmod +x tldr-bash
COPY --from=node-builder /build-outputs/node /build-outputs/tldr-node ./
COPY --from=node-builder /build-outputs/node_modules/ ./node_modules/
COPY --from=zig-builder /build-outputs/* ./

# python is really hard to isolate in a package, using pyinstaller didn't really work either, so for now we just use it like this
RUN python3 -m venv tldr-python \
        && cd tldr-python \
        && bash -c 'source bin/activate; pip install wheel; pip install tldr; deactivate' \
        && cd .. \
        && echo '#!/bin/bash' > tldr-python.bash \
        && echo 'source tldr-python/bin/activate; tldr $@' >> tldr-python.bash \
        && chmod +x tldr-python.bash

# Update all the individual caches
RUN bash -c 'mkdir -p /caches/{tealdeer,tldr-c,tldr-hs,fast-tldr,tldr-bash,tldr-node,tldr-python,outfieldr/.local/share}' \
        && TEALDEER_CACHE_DIR=/caches/tealdeer ./tealdeer -u \
        && TLDR_CACHE_DIR=/caches/tldr-c ./tldr-c -u \
        && XDG_DATA_HOME=/caches/tldr-hs ./tldr-hs -u \
        && XDG_DATA_HOME=/caches/fast-tldr ./fast-tldr -u \
        && XDG_DATA_HOME=/caches/tldr-bash ./tldr-bash -u \
        && HOME=/caches/tldr-node ./tldr-node -u \
        && HOME=/caches/tldr-python ./tldr-python.bash -u \
        && HOME=/caches/outfieldr ./outfieldr -u

WORKDIR /tools
RUN wget -q https://github.com/sharkdp/hyperfine/releases/download/v1.11.0/hyperfine_1.11.0_amd64.deb && dpkg -i hyperfine_1.11.0_amd64.deb

ENV PAGE="tar"
WORKDIR /clients
CMD hyperfine \
        --warmup 10 \
        --runs 50 \
        --prepare 'sync; echo 3 | tee /proc/sys/vm/drop_caches' \
        "TEALDEER_CACHE_DIR=/caches/tealdeer ./tealdeer $PAGE" \
        "TLDR_CACHE_DIR=/caches/tldr-c ./tldr-c $PAGE" \
        "XDG_DATA_HOME=/caches/tldr-hs ./tldr-hs $PAGE" \
        "XDG_DATA_HOME=/caches/fast-tldr ./fast-tldr $PAGE" \
        "XDG_DATA_HOME=/caches/tldr-bash TLDR_LESS=0 ./tldr-bash $PAGE" \
        "HOME=/caches/tldr-python ./tldr-python.bash $PAGE" \
        "HOME=/caches/outfieldr ./outfieldr $PAGE" \
        "HOME=/caches/tldr-node ./tldr-node $PAGE"

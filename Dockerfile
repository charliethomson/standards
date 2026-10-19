# syntax=docker/dockerfile:1
# The standards server image: ghcr.io/charliethomson/standards-server (bin
# `standards-server`, dev.thmsn.standards.server). From templates/docker/Dockerfile.rust.
#
# Build context is the REPO ROOT, not a crate dir: standards-db embeds docs/, skills/,
# templates/ and CHANGELOG.md at compile time (rust-embed), so the served content is
# baked into the binary. .dockerignore is an allowlist of what the build reads.
#
#   docker build --ssh default -t standards-server:dev .
#
# CI: .woodpecker/server.build.yml via scripts/ci/docker-build.sh.

# Stage 1: build with a pinned Rust toolchain (edition 2024).
FROM rust:1-bookworm AS build
WORKDIR /build

# liblog is a private crate fetched over SSH (Cargo.toml). The key is supplied at build
# time via `--mount=type=ssh`, never baked into the image: CI forwards its agent with
# `--ssh default`; locally, run `docker build --ssh default .` with your agent loaded.
RUN apt-get update \
    && apt-get install -y --no-install-recommends openssh-client \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p -m 0700 /root/.ssh \
    && ssh-keyscan github.com >> /root/.ssh/known_hosts

# Cap parallel rustc as .woodpecker/rust.ci.yml does: this compile runs in the agent
# host's BuildKit, which no step env or container limit reaches, and the 2G/4G agents
# OOM-kill an uncapped build. Below the apt layer so that stays cached.
ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS

COPY . .

# Release version (docs/versioning.md). The context has no .git, so build.rs can't
# derive it; CI passes it (scripts/ci/version.sh). Unset, the binary reports 0.0.0.
ARG RELEASE_VERSION=""
# Build info and content identity (docs/build-info.md). No .git here either, so
# libbuildinfo records nothing and standards-db takes the commit from RELEASE_COMMIT:
# it is the ETag and the manifest's commit. Unset (a local build), the commit is
# `unknown` and content routes send no ETag. RELEASE_DIRTY / RELEASE_COMMIT_TIME are
# what git said about the checkout, passed for parity with the fleet's builds.
ARG RELEASE_COMMIT=""
ARG RELEASE_DIRTY=""
ARG RELEASE_COMMIT_TIME=""
ENV RELEASE_VERSION=$RELEASE_VERSION \
    RELEASE_COMMIT=$RELEASE_COMMIT \
    RELEASE_DIRTY=$RELEASE_DIRTY \
    RELEASE_COMMIT_TIME=$RELEASE_COMMIT_TIME

# The download caches are BuildKit cache mounts (bounded by docker-build.sh's
# `builder prune --keep-storage`); the target dir is not, so the binary is in a layer.
RUN --mount=type=ssh \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release --locked -p standards-api --bin standards-server

# Stage 2: slim runtime image.
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 standards \
    && useradd --system --uid 10001 --gid standards --no-create-home \
        --home-dir /var/lib/standards --shell /usr/sbin/nologin standards \
    && install -d -o standards -g standards -m 0750 /var/lib/standards

COPY --from=build /build/target/release/standards-server /usr/local/bin/standards-server

# Stateless: the content is in the binary and nothing is written but liblog's
# size-capped JSON log file, which goes under LIBPATH_BASE_DIR (main.rs turns off
# libpath's forced mkdir, docs/configuration.md). No volume: OTLP is the log of record.
ENV LIBPATH_BASE_DIR=/var/lib/standards
ENV STANDARDS_BIND=0.0.0.0:8080
EXPOSE 8080

USER standards:standards

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/standards-server"]

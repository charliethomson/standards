#!/usr/bin/env sh
# Build and push an image to ghcr. Copied from vision's scripts/ci/docker-build.sh;
# only the comments differ.
#
# Usage:  scripts/ci/docker-build.sh <image> <dockerfile> <context> [extra docker build args...]
#   e.g.  scripts/ci/docker-build.sh standards-server Dockerfile . --ssh default
#
# The context is the repo root: standards-db embeds docs/, skills/, templates/ and
# CHANGELOG.md, so the build needs them. The root .dockerignore is an allowlist that
# keeps the context to those plus the workspace (no .git, no target/).
#
# Env: GHCR_TOKEN (push + cache pull), CI_PIPELINE_EVENT, CI_COMMIT_SHA.
set -eu

image="${1:?docker-build.sh needs an image name}"
dockerfile="${2:?docker-build.sh needs a dockerfile}"
context="${3:?docker-build.sh needs a build context}"
shift 3

repo="ghcr.io/charliethomson/$image"

# Canonical version, stamped into the binary (the builder can't rely on git).
# NB the nested substitution: this must capture the script's output, not just
# execute it.
eval "$("$(dirname "$0")/version.sh")"
echo "Building $image $VERSION (event=${CI_PIPELINE_EVENT:-unknown})"

export DOCKER_BUILDKIT=1

# Log in before building so --cache-from can read the private cache image. Each
# agent otherwise keeps a separate local layer cache, so a build landing on the
# other agent starts cold.
#
# This replaces `cache-{from,to}: type=gha`, which the GitHub workflows used and
# which has no meaning off GitHub — the gha cache backend is the Actions cache
# service. Registry-inline cache is the portable equivalent.
if [ -n "${GHCR_TOKEN:-}" ]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u charliethomson --password-stdin >/dev/null
fi

# BUILDKIT_INLINE_CACHE embeds cache metadata in the pushed image, which is what
# makes :main usable as --cache-from on the *other* agent.
docker build \
  -f "$dockerfile" \
  --build-arg RELEASE_VERSION="$VERSION" \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --cache-from "$repo:main" \
  -t "$repo:main" \
  -t "$repo:sha-$CI_COMMIT_SHA" \
  "$@" \
  "$context"

if [ "${CI_PIPELINE_EVENT:-}" != "pull_request" ]; then
  docker push "$repo:main"
  docker push "$repo:sha-$CI_COMMIT_SHA"
else
  echo "pull_request: built only, not pushed"
fi

# Housekeeping. Every build produces a :sha-<commit> tag, and a *tagged* image is
# never reclaimed by `docker system prune` — so they accumulate and quietly fill
# the agent. The sha tag lives in the registry; only :main is needed locally, as
# the --cache-from source.
docker rmi "$repo:sha-$CI_COMMIT_SHA" >/dev/null 2>&1 || true
docker image prune -f >/dev/null 2>&1 || true
docker builder prune -f --keep-storage 5GB >/dev/null 2>&1 || true
echo "disk after build:"
df -h /var/lib/docker 2>/dev/null | tail -1 || df -h / | tail -1

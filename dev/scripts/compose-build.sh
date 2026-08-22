#!/usr/bin/env bash
#
# Build a compose project's images, narrowed to (ONLY) or excluding (SKIP) specific services.
# `docker compose up --build` rebuilds every service, which is expensive here: the Camoufox
# image downloads a browser and the API image compiles the Rust workspace.
#
# Usage:
#   ONLY=secutils_webui ./dev/scripts/compose-build.sh -f a.yml -f b.yml --env-file .env
#   SKIP=retrack_web_scraper_camoufox ./dev/scripts/compose-build.sh -f a.yml --env-file .env
#
set -euo pipefail

if [[ -n "${ONLY:-}" && -n "${SKIP:-}" ]]; then
  echo "compose-build: set either ONLY or SKIP, not both" >&2
  exit 1
fi

all="$(docker compose "$@" config --services)"

# A typo in SKIP would silently rebuild everything, which is the wait this flag exists to avoid.
for name in $(tr ',' ' ' <<<"${ONLY:-} ${SKIP:-}"); do
  if ! grep -qxF "$name" <<<"$all"; then
    echo "compose-build: unknown service '$name'. Known services:" >&2
    sed 's/^/  /' <<<"$all" >&2
    exit 1
  fi
done

if [[ -n "${ONLY:-}" ]]; then
  targets="${ONLY//,/ }"
else
  targets="$all"
  for name in $(tr ',' ' ' <<<"${SKIP:-}"); do
    targets="$(grep -vxF "$name" <<<"$targets")"
  done
  targets="$(tr '\n' ' ' <<<"$targets")"
fi

echo "compose-build: building ${targets}"

# Services without a build section are ignored by compose ("No services to build"), so the
# full service list can be passed as-is.
# shellcheck disable=SC2086
docker compose "$@" build $targets

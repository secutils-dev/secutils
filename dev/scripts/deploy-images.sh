#!/usr/bin/env bash
# Build and publish images with local Docker or a dedicated SSH Buildx builder.
set -euo pipefail

ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PLATFORM="${DEPLOY_PLATFORM:-linux/arm64}"
BUILDER="${DEPLOY_REMOTE_BUILDER:-secutils-deploy}"
REMOTE_DIR="${DEPLOY_REMOTE_DIR:-/home/ubuntu/secutils-build}"
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=20 "${DEPLOY_REMOTE_HOST:-}")
# This configuration is a command prefix, split into arguments without eval.
read -r -a REMOTE_DOCKER <<<"${DEPLOY_REMOTE_DOCKER:-sudo -n docker}"
REMOTE_ENV=()
TEMPORARY=""
LOCAL_TEMP=""
BUILD_STARTED=0

fail() { printf 'deploy: %s\n' "$*" >&2; exit 1; }
require() { [[ -n "${!1:-}" ]] || fail "Set $1 in deploy.env or the environment."; }
quote() { printf '%q ' "$@"; }
remote() { "${SSH[@]}" "$(quote "$@")"; }
engine() { remote "${REMOTE_DOCKER[@]}" "${REMOTE_ENV[@]}" "$@"; }

image_paths() {
  case "$1" in
    secutils-api) CONTEXT=.; DOCKERFILE=Dockerfile ;;
    secutils-webui) CONTEXT=.; DOCKERFILE=Dockerfile.webui ;;
    secutils-docs) CONTEXT=.; DOCKERFILE=Dockerfile.docs ;;
    retrack-api) CONTEXT=components/retrack; DOCKERFILE=Dockerfile ;;
    retrack-web-scraper) CONTEXT=components/retrack; DOCKERFILE=Dockerfile.web-scraper ;;
    retrack-web-scraper-camoufox) CONTEXT=components/retrack; DOCKERFILE=Dockerfile.web-scraper-camoufox ;;
    *) fail "Unknown image: $1" ;;
  esac
}

health() {
  require DEPLOY_REMOTE_HOST
  # The remote shell reads Docker's actual filesystem and Linux memory counters.
  "${SSH[@]}" "bash -s -- $(quote "${REMOTE_DOCKER[@]}")" <<'REMOTE'
set -euo pipefail
root="$("$@" info --format '{{.DockerRootDir}}')"
"$@" info --format 'cpus={{.NCPU}} memory_total_bytes={{.MemTotal}} architecture={{.Architecture}}'
sudo -n df -Pk "$root" | awk 'NR==2 {printf "disk_free_bytes=%.0f disk_total_bytes=%.0f\n", $4*1024, $2*1024}'
awk '/^MemAvailable:/ {printf "memory_available_bytes=%.0f\n", $2*1024} /^SwapTotal:/ {printf "swap_total_bytes=%.0f\n", $2*1024}' /proc/meminfo
printf 'docker_root=%s\n' "$root"
"$@" system df
REMOTE
}

health_json() {
  health | jq -Rs '{report: ., disk_free_bytes: (capture("disk_free_bytes=(?<n>[0-9]+)").n|tonumber), memory_available_bytes: (capture("memory_available_bytes=(?<n>[0-9]+)").n|tonumber)}'
}

tracked_files() {
  local directory="$1" prefix="$2" entry metadata name
  while IFS= read -r -d '' entry; do
    metadata="${entry%%$'\t'*}"
    name="${entry#*$'\t'}"
    if [[ "$metadata" == 160000\ * ]]; then
      tracked_files "$directory/$name" "$prefix$name/"
    elif [[ -e "$directory/$name" || -L "$directory/$name" ]]; then
      printf '%s\0' "$prefix$name"
    fi
  done < <(git -C "$directory" ls-files --stage -z)
}

registry_auth() {
  local config="${DOCKER_CONFIG:-$HOME/.docker}/config.json" helper credentials
  helper="$(jq -r --arg registry "$DEPLOY_REGISTRY" '.credHelpers[$registry] // .credsStore // empty' "$config")"
  if [[ -n "$helper" ]]; then
    credentials="$(printf '%s\n' "$DEPLOY_REGISTRY" | "docker-credential-$helper" get)"
    jq -c --arg registry "$DEPLOY_REGISTRY" '
      {auths: {($registry): (if .Username == "<token>" then {identitytoken: .Secret}
        else {auth: ((.Username + ":" + .Secret)|@base64)} end)}}' <<<"$credentials"
  else
    jq -ce --arg registry "$DEPLOY_REGISTRY" '
      .auths[$registry] | select(.auth or .identitytoken) | {auths: {($registry): .}}' "$config" ||
      fail "Run docker login $DEPLOY_REGISTRY before a remote push."
  fi
}

cleanup() {
  local code=$? cleanup_code=0 now push_seconds
  trap - EXIT
  if [[ -n "$TEMPORARY" ]]; then
    if (( BUILD_STARTED )); then
      engine buildx stop "$BUILDER" || cleanup_code=1
      health_json >"$LOCAL_TEMP/health-after.json" || cleanup_code=1
    fi
    remote sudo -n rm -rf -- "$TEMPORARY" || {
      printf 'deploy: cleanup failed. Remove remote directory %s after SSH recovers.\n' "$TEMPORARY" >&2
      cleanup_code=1
    }
  fi
  if [[ -n "$LOCAL_TEMP" ]]; then
    now="$(date +%s)"
    push_seconds="$(awk '/pushing (layers|manifest)/ && /[0-9.]+s done$/ {n=$(NF-1); sub(/s$/, "", n); total+=n} END {print total+0}' "$LOG")"
    jq --argjson total "$((now-START))" --argjson push "$push_seconds" --argjson code "$code" \
      --argjson cleanup "$cleanup_code" --slurpfile after "$LOCAL_TEMP/health-after.json" \
      '. + {total_seconds: $total, push_seconds: $push, exit_code: $code, cleanup_exit_code: $cleanup}
       + (if ($after|length)>0 then {remote_health_after: $after[0]} else {} end)' \
      "$LOCAL_TEMP/record.json" | jq -c . | tee -a "$ROOT/.deploy/history.jsonl"
    rm -rf -- "$LOCAL_TEMP"
  fi
  (( code != 0 )) || code=$cleanup_code
  exit "$code"
}

publish() {
  local image="${1:-}" route="${DEPLOY_BUILD_ROUTE:-auto}" selected dry_run=0 tag
  local before upload_start build_start code args command dirty=false
  local tar_options=()
  image_paths "$image"
  shift
  require DEPLOY_REGISTRY
  args=()
  for tag in "$@"; do
    if [[ "$tag" == --dry-run ]]; then dry_run=1; continue; fi
    [[ "$tag" =~ ^[a-zA-Z0-9_][a-zA-Z0-9_.-]*$ && ${#tag} -le 128 ]] || fail "Invalid Docker image tag: $tag"
    args+=(--tag "$DEPLOY_REGISTRY/$image:$tag")
  done
  (( ${#args[@]} > 0 )) || fail "Supply at least one image tag."
  case "$route" in auto|local|remote) ;; *) fail "DEPLOY_BUILD_ROUTE must be auto, local, or remote." ;; esac
  [[ "$route" != auto ]] || route=local
  selected="${DEPLOY_REMOTE_IMAGES:-}"
  for selected in ${selected//,/ }; do
    image_paths "$selected"
    if [[ "${DEPLOY_BUILD_ROUTE:-auto}" == auto && "$selected" == "$image" ]]; then route=remote; fi
  done
  image_paths "$image"
  [[ "$route" != remote ]] || require DEPLOY_REMOTE_HOST
  if (( dry_run )); then
    printf 'image=%s route=%s context=%s dockerfile=%s tags=%s\n' "$image" "$route" "$CONTEXT" "$DOCKERFILE" "${args[*]}"
    return
  fi

  START="$(date +%s)"
  umask 077
  mkdir -p "$ROOT/.deploy"
  LOCAL_TEMP="$(mktemp -d "$ROOT/.deploy/run.XXXXXXXX")"
  LOG="$ROOT/.deploy/$START-$(basename "$LOCAL_TEMP")-$image.log"
  touch "$LOG" "$LOCAL_TEMP/health-after.json"
  [[ -z "$(git -C "$ROOT/$CONTEXT" status --porcelain)" ]] || dirty=true
  jq -n --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg image "$image" \
    --arg route "$route" --arg platform "$PLATFORM" --arg log "${LOG#"$ROOT/"}" \
    --arg commit "$(git -C "$ROOT/$CONTEXT" rev-parse HEAD)" --argjson dirty "$dirty" \
    --argjson tags "$(printf '%s\n' "${args[@]}" | jq -Rs 'split("\n")|map(select(. != "--tag" and . != ""))')" \
    '{timestamp:$timestamp,image:$image,route:$route,tags:$tags,platform:$platform,log:$log,commit:$commit,dirty:$dirty}' >"$LOCAL_TEMP/record.json"
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  command=(buildx build --progress=plain --push --platform "$PLATFORM" "${args[@]}")
  if [[ "$route" == remote ]]; then
    before="$(health_json)"
    jq --argjson health "$before" '. + {remote_health_before:$health}' "$LOCAL_TEMP/record.json" >"$LOCAL_TEMP/updated.json"
    mv "$LOCAL_TEMP/updated.json" "$LOCAL_TEMP/record.json"
    (( $(jq -r .disk_free_bytes <<<"$before") >= ${DEPLOY_REMOTE_MIN_FREE_GB:-8}*1024*1024*1024 )) ||
      fail "Remote Docker filesystem has insufficient free space. Run make deploy-health."
    if [[ "$image" == *-api ]]; then
      (( $(jq -r .memory_available_bytes <<<"$before") >= 4*1024*1024*1024 )) ||
        fail "Remote Rust builds require 4 GiB available RAM. Select DEPLOY_BUILD_ROUTE=local."
    fi
    registry_auth >"$LOCAL_TEMP/auth.json"
    remote ls -ld "$(dirname "$REMOTE_DIR")" >/dev/null
    remote mkdir -p "$REMOTE_DIR"
    TEMPORARY="$(remote mktemp -d "$REMOTE_DIR/run.XXXXXXXX")"
    remote mkdir -m 700 "$TEMPORARY/source" "$TEMPORARY/docker"
    remote tee "$TEMPORARY/docker/config.json" <"$LOCAL_TEMP/auth.json" >/dev/null
    remote chmod 600 "$TEMPORARY/docker/config.json"
    upload_start="$(date +%s)"
    tracked_files "$ROOT/$CONTEXT" "" >"$LOCAL_TEMP/files"
    if [[ "$(tar --version)" == *bsdtar* ]]; then
      tar_options=(--no-xattrs --no-acls --no-fflags)
    fi
    COPYFILE_DISABLE=1 tar "${tar_options[@]}" -C "$ROOT/$CONTEXT" --no-recursion --null -T "$LOCAL_TEMP/files" -cf - |
      remote tar -xf - -C "$TEMPORARY/source"
    jq --argjson upload "$(( $(date +%s)-upload_start ))" '. + {context_upload_seconds:$upload}' \
      "$LOCAL_TEMP/record.json" >"$LOCAL_TEMP/updated.json"
    mv "$LOCAL_TEMP/updated.json" "$LOCAL_TEMP/record.json"
    REMOTE_ENV=(env "DOCKER_CONFIG=$TEMPORARY/docker" "BUILDX_CONFIG=$REMOTE_DIR/buildx")
    # Place env before Docker, retaining a possible sudo prefix.
    local docker_index=$((${#REMOTE_DOCKER[@]}-1))
    REMOTE_DOCKER=("${REMOTE_DOCKER[@]:0:docker_index}" "${REMOTE_ENV[@]}" "${REMOTE_DOCKER[docker_index]}")
    REMOTE_ENV=()
    if ! engine buildx inspect "$BUILDER" >/dev/null 2>&1; then
      engine buildx create --name "$BUILDER" --driver docker-container \
        --driver-opt "memory=${DEPLOY_REMOTE_MEMORY:-2g}" \
        --driver-opt "memory-swap=${DEPLOY_REMOTE_MEMORY:-2g}" \
        --driver-opt "cpu-quota=${DEPLOY_REMOTE_CPU_QUOTA:-100000}"
    fi
    BUILD_STARTED=1
    engine buildx inspect "$BUILDER" --bootstrap >/dev/null
    engine buildx du --builder "$BUILDER" >"$LOCAL_TEMP/cache"
    jq --rawfile cache "$LOCAL_TEMP/cache" '. + {builder_cache_before:$cache}' \
      "$LOCAL_TEMP/record.json" >"$LOCAL_TEMP/updated.json"
    mv "$LOCAL_TEMP/updated.json" "$LOCAL_TEMP/record.json"
    command=("${SSH[@]}" "$(quote "${REMOTE_DOCKER[@]}" "${command[@]}" --builder "$BUILDER" -f "$TEMPORARY/source/$DOCKERFILE" "$TEMPORARY/source")")
  else
    command=(docker "${command[@]}" -f "$ROOT/$CONTEXT/$DOCKERFILE" "$ROOT/$CONTEXT")
  fi
  build_start="$(date +%s)"
  set +e
  "${command[@]}" 2>&1 | tee "$LOG"
  code=${PIPESTATUS[0]}
  set -e
  jq --argjson duration "$(( $(date +%s)-build_start ))" \
    '. + {build_push_seconds:$duration}' "$LOCAL_TEMP/record.json" >"$LOCAL_TEMP/updated.json"
  mv "$LOCAL_TEMP/updated.json" "$LOCAL_TEMP/record.json"
  exit "$code"
}

command -v jq >/dev/null || fail "Install jq to use image publishing commands."
case "${1:-}" in
  publish) shift; publish "$@" ;;
  health) health ;;
  stats)
    if [[ -f "$ROOT/.deploy/history.jsonl" ]]; then
      jq -r '[.timestamp,.image,.route,"total=\(.total_seconds)s","build+push=\(.build_push_seconds // 0)s","push=\(.push_seconds // 0)s","exit=\(.exit_code // "preflight failure")"]|join(" ")' "$ROOT/.deploy/history.jsonl"
    fi ;;
  *) fail "Usage: bash dev/scripts/deploy-images.sh publish IMAGE TAG... [--dry-run] | health | stats" ;;
esac

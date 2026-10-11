# shellcheck shell=bash
# Detects a working Compose command. Sourced by scripts/dev.sh and scripts/showcase.sh.
#
# Preference order: `docker compose`, `podman compose`, `podman-compose`, `docker-compose`.
# Each candidate is accepted only when `<candidate> version` actually succeeds, so a
# container runtime without a Compose plugin is skipped.

COMPOSE_CMD=()

detect_compose_command() {
  COMPOSE_CMD=()
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
  elif command -v podman >/dev/null 2>&1 && podman compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(podman compose)
  elif command -v podman-compose >/dev/null 2>&1 && podman-compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(podman-compose)
  elif command -v docker-compose >/dev/null 2>&1 && docker-compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
  else
    return 1
  fi
}

# True when containers run under rootless Podman (podman itself, the podman-docker
# shim, or a Docker client pointed at a Podman socket).
compose_uses_rootless_podman() {
  local podman_backed=0
  case "${COMPOSE_CMD[0]:-}" in
    podman | podman-compose) podman_backed=1 ;;
  esac
  if [[ "$podman_backed" -eq 0 ]]; then
    case "${DOCKER_HOST:-}" in
      *podman*) podman_backed=1 ;;
    esac
  fi
  if [[ "$podman_backed" -eq 0 ]] && docker --version 2>/dev/null | grep -qi podman; then
    podman_backed=1
  fi
  [[ "$podman_backed" -eq 1 ]] || return 1
  command -v podman >/dev/null 2>&1 || return 1
  [[ "$(podman info --format '{{.Host.Security.Rootless}}' 2>/dev/null)" == "true" ]]
}

# Container user for the compose services. The compose file runs them as
# HOST_UID:HOST_GID so bind-mounted data stays owned by the host user. Under rootless
# Podman the host user is container root, so the default becomes 0:0 there.
compose_host_ids() {
  local default_uid default_gid
  default_uid="$(id -u)"
  default_gid="$(id -g)"
  if [[ -z "${HOST_UID:-}" || -z "${HOST_GID:-}" ]] && compose_uses_rootless_podman; then
    default_uid=0
    default_gid=0
  fi
  COMPOSE_HOST_UID="${HOST_UID:-$default_uid}"
  COMPOSE_HOST_GID="${HOST_GID:-$default_gid}"
}

# Runs the detected Compose command with the host user mapping the compose file expects.
compose() {
  if [[ "${#COMPOSE_CMD[@]}" -eq 0 ]]; then
    detect_compose_command || {
      printf 'missing: compose (docker compose, podman compose or podman-compose)\n' >&2
      return 127
    }
  fi
  compose_host_ids
  HOST_UID="$COMPOSE_HOST_UID" HOST_GID="$COMPOSE_HOST_GID" "${COMPOSE_CMD[@]}" "$@"
}

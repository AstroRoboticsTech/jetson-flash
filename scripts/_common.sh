#!/usr/bin/env bash
# Shared logging for the jetson-flash pipeline. Source this, then call
# `log_init <step>` near the top of each script. Repo-level messages get
# color on a TTY; every step also streams to logs/<step>-<ts>.log (and a
# logs/<step>-latest.log symlink) with ANSI stripped so the file is plain.

_jf_repo_root() {
    local d
    d="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    printf '%s' "$d"
}

if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    C_RESET=$'\e[0m'; C_STEP=$'\e[1;35m'; C_INFO=$'\e[36m'
    C_OK=$'\e[1;32m'; C_WARN=$'\e[1;33m'; C_ERR=$'\e[1;31m'
else
    C_RESET=''; C_STEP=''; C_INFO=''; C_OK=''; C_WARN=''; C_ERR=''
fi

log_step() { echo "${C_STEP}==>${C_RESET} $*"; }
log_info() { echo "${C_INFO}[info]${C_RESET} $*"; }
log_ok()   { echo "${C_OK}[ok]${C_RESET} $*"; }
log_warn() { echo "${C_WARN}[warn]${C_RESET} $*" >&2; }
log_err()  { echo "${C_ERR}[fail]${C_RESET} $*" >&2; }

# log_init <step-name>: redirect stdout+stderr through tee so the terminal
# keeps color and logs/<step>-<ts>.log gets a plain-text copy.
log_init() {
    local step="${1:?log_init needs a step name}"
    local dir="${LOG_DIR:-$(_jf_repo_root)/logs}"
    mkdir -p "$dir"
    local ts file
    ts="$(date +%Y%m%d-%H%M%S)"
    file="$dir/${step}-${ts}.log"
    ln -sf "$(basename "$file")" "$dir/${step}-latest.log"
    exec > >(tee >(sed -u 's/\x1b\[[0-9;]*m//g' >> "$file")) 2>&1
    log_info "logging $step -> ${file#"$(_jf_repo_root)"/}"
}

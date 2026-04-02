#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/deploy-rpi.sh --host HOST [options] [-- REMOTE_ARGS...]

Cross-build the raspi_stream binary, copy it to Raspberry Pi over SSH, and
optionally run it on the target.

Options:
  --host HOST         Raspberry Pi host name or IP address
  --user USER         SSH user name (default: pi)
  --port PORT         SSH port (default: 22)
  --remote-dir DIR    Remote directory for the deployed binary
                      (default: /home/pi/raspi_stream)
  --target TRIPLE     Cargo target triple
                      (default: aarch64-unknown-linux-gnu)
  --bin-name NAME     Binary name to deploy (default: raspi_stream)
  --release           Build the release profile (default)
  --debug             Build the debug profile
  --run               Run the deployed binary over SSH after copying
  --help              Show this help

Environment:
  RPI_SSH_OPTS        Extra options passed to ssh/scp, for example:
                      RPI_SSH_OPTS='-i ~/.ssh/id_rsa'

Examples:
  scripts/deploy-rpi.sh --host raspberrypi.local
  scripts/deploy-rpi.sh --host 192.168.1.50 --remote-dir /opt/raspi_stream
  scripts/deploy-rpi.sh --host raspberrypi.local --run -- --source libcamera --host 0.0.0.0
EOF
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: required command not found: $1" >&2
        exit 1
    fi
}

project_root() {
    local script_dir

    script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
    cd -- "$script_dir/.." && pwd
}

target_host=""
target_user="pi"
target_port="22"
target_triple="aarch64-unknown-linux-gnu"
bin_name="raspi_stream"
build_profile="release"
run_after_deploy=0
remote_dir=""
remote_args=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)
            target_host="${2:-}"
            shift 2
            ;;
        --user)
            target_user="${2:-}"
            shift 2
            ;;
        --port)
            target_port="${2:-}"
            shift 2
            ;;
        --remote-dir)
            remote_dir="${2:-}"
            shift 2
            ;;
        --target)
            target_triple="${2:-}"
            shift 2
            ;;
        --bin-name)
            bin_name="${2:-}"
            shift 2
            ;;
        --release)
            build_profile="release"
            shift
            ;;
        --debug)
            build_profile="debug"
            shift
            ;;
        --run)
            run_after_deploy=1
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --)
            shift
            remote_args=("$@")
            break
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -z "$target_host" ]]; then
    echo "error: --host is required" >&2
    usage >&2
    exit 1
fi

if [[ -z "$remote_dir" ]]; then
    remote_dir="/home/${target_user}/raspi_stream"
fi

require_command cargo
require_command ssh
require_command scp

repo_root="$(project_root)"
profile_dir="$build_profile"
local_binary="$repo_root/target/$target_triple/$profile_dir/$bin_name"
ssh_target="${target_user}@${target_host}"
remote_binary="$remote_dir/$bin_name"

ssh_extra_opts=()
if [[ -n "${RPI_SSH_OPTS:-}" ]]; then
    # shellcheck disable=SC2206
    ssh_extra_opts=(${RPI_SSH_OPTS})
fi

ssh_opts=(-p "$target_port")
scp_opts=(-P "$target_port")
ssh_opts+=("${ssh_extra_opts[@]}")
scp_opts+=("${ssh_extra_opts[@]}")

echo "==> deploying raspi_stream"
echo "  host         : $target_host"
echo "  user         : $target_user"
echo "  port         : $target_port"
echo "  target       : $target_triple"
echo "  profile      : $build_profile"
echo "  remote dir   : $remote_dir"
echo "  binary       : $bin_name"
if [[ "$run_after_deploy" -eq 1 ]]; then
    echo "  run          : yes"
else
    echo "  run          : no"
fi

build_cmd=(cargo build --target "$target_triple" --bin "$bin_name")
if [[ "$build_profile" == "release" ]]; then
    build_cmd+=(--release)
fi

echo "==> building binary"
(
    cd -- "$repo_root"
    "${build_cmd[@]}"
)

if [[ ! -f "$local_binary" ]]; then
    echo "error: build succeeded but binary was not found: $local_binary" >&2
    exit 1
fi

echo "==> preparing remote directory"
# shellcheck disable=SC2029
ssh "${ssh_opts[@]}" "$ssh_target" "mkdir -p $(printf '%q' "$remote_dir")"

echo "==> copying binary"
scp "${scp_opts[@]}" "$local_binary" "$ssh_target:$remote_binary"

echo "==> finalizing remote binary"
# shellcheck disable=SC2029
ssh "${ssh_opts[@]}" "$ssh_target" "chmod +x $(printf '%q' "$remote_binary")"

echo "deployment complete"
echo "  binary path  : $remote_binary"

if [[ "$run_after_deploy" -eq 1 ]]; then
    remote_cmd=("$remote_binary")
    if [[ "${#remote_args[@]}" -gt 0 ]]; then
        remote_cmd+=("${remote_args[@]}")
    fi

    printf -v remote_cmd_string '%q ' "${remote_cmd[@]}"

    echo "==> running remote binary"
    ssh -tt "${ssh_opts[@]}" "$ssh_target" "cd $(printf '%q' "$remote_dir") && ${remote_cmd_string% }"
fi
#!/usr/bin/env bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# Affero General Public License as published by the Free Software Foundation,
# version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
# details.
#
# You should have received a copy of the GNU Affero General Public License along
# with this program. If not, see <https://www.gnu.org/licenses/>.

# Native development stack: daemon, web host, Vite, optional telnet and workers.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

usage() {
    cat <<'EOF'
Usage: scripts/dev.sh [OPTIONS]

Starts the split-process daemon, web host, and Meadow's Vite dev server.
Ctrl-C stops the whole stack. If a service exits, the other services stop too.

Options:
  --telnet                   Start the telnet host on 0.0.0.0:8888
  --curl-worker              Start the HTTP worker
  --git-worker               Start the Git worker
  --core PATH                Objdef core directory (default: cores/cowbell/src)
  --git-upstream REF         Git baseline branch (default: origin/main)
  --baseline-objdef-dir PATH  Use this baseline directory instead of Git history
  --data-dir PATH            Database, keys, and IPC directory (default: moor-data)
  --clean                    Wipe the data directory before startup (fresh core import)
  --debug                    Build debug binaries (default)
  --release                  Build release binaries
  --help                     Show this help

Environment:
  MOOR_CORE         Core directory; overridden by --core
  MOOR_DATA_DIR     Data directory; overridden by --data-dir
  MOOR_DB           Database filename (default: development.db)
  MOOR_EXPORT       Checkpoint export directory (default: development-export)
  CARGO_TARGET_DIR  Cargo build directory
  OPENROUTER_KEY    Configure Cowbell's shared LLM client and start the HTTP worker
  OPENROUTER_API_KEY  Alternative name for OPENROUTER_KEY (OPENROUTER_KEY takes precedence)

Examples:
  scripts/dev.sh --telnet
  scripts/dev.sh --curl-worker --git-worker
  scripts/dev.sh --clean --curl-worker --git-worker
  MOOR_CORE=cores/snore/src scripts/dev.sh --curl-worker

Existing databases are reused unless --clean is given.
The core and its baseline are imported only for a new database.
By default, the baseline comes from the common ancestor of HEAD and origin/main.
Working files supply the live code, so local edits remain distinct from upstream.
Git preparation uses local history; it does not fetch from the remote.
With OPENROUTER_KEY or OPENROUTER_API_KEY, each launch configures Cowbell for deepseek/deepseek-v4.1-flash,
enables Mr. Welcome and player assistance, and permits shared provider use.
Meadow: http://localhost:3000    Web host: http://localhost:8080
Both web servers listen on all IPv4 interfaces. From another machine, use
http://<this-machine-IP>:3000, including this machine's tailnet IP.
Worker IPC sockets are printed at startup so other workers can attach.
EOF
}

core=${MOOR_CORE:-cores/cowbell/src}
data_dir=${MOOR_DATA_DIR:-moor-data}
profile=debug
telnet=false
curl_worker=false
git_worker=false
clean=false
git_upstream=
baseline_objdef_dir=

while (($#)); do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --telnet) telnet=true; shift ;;
        --curl-worker) curl_worker=true; shift ;;
        --git-worker) git_worker=true; shift ;;
        --clean) clean=true; shift ;;
        --debug) profile=debug; shift ;;
        --release) profile=release; shift ;;
        --core|--data-dir|--git-upstream|--baseline-objdef-dir)
            if (($# < 2)) || [[ -z $2 || $2 == --* ]]; then
                echo "Missing value for $1" >&2
                exit 2
            fi
            case "$1" in
                --core) core=$2 ;;
                --data-dir) data_dir=$2 ;;
                --git-upstream) git_upstream=$2 ;;
                --baseline-objdef-dir) baseline_objdef_dir=$2 ;;
            esac
            shift 2
            ;;
        *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
    esac
done

if [[ -n $git_upstream && -n $baseline_objdef_dir ]]; then
    echo "Choose either --git-upstream or --baseline-objdef-dir" >&2
    exit 2
fi
if [[ -z $git_upstream && -z $baseline_objdef_dir ]]; then
    git_upstream=origin/main
fi

for tool in cargo node npm; do
    command -v "$tool" >/dev/null || { echo "Required command not found: $tool" >&2; exit 1; }
done
if [[ ! -d $core ]]; then
    echo "Core directory not found: $core" >&2
    exit 1
fi
startup_config_dir=
# Removes the key file if the launcher fails before the daemon reads it.
trap 'if [[ -n $startup_config_dir ]]; then rm -rf -- "$startup_config_dir"; fi' EXIT
if [[ -n ${OPENROUTER_KEY:-${OPENROUTER_API_KEY:-}} && -f $core/llm/llm_client.moo ]]; then
    startup_config_dir=$(mktemp -d "${TMPDIR:-/tmp}/moor-dev-llm.XXXXXX")
    (umask 077; node - "$startup_config_dir/config.json" <<'JS'
const fs = require("node:fs");
fs.writeFileSync(process.argv[2], JSON.stringify({llm: {
    endpoint: "https://openrouter.ai/api/v1/chat/completions",
    model: "deepseek/deepseek-v4.1-flash",
    key: process.env.OPENROUTER_KEY || process.env.OPENROUTER_API_KEY,
}}), {mode: 0o600});
JS
    )
    curl_worker=true
    printf 'LLM: configuring shared OpenRouter access with deepseek/deepseek-v4.1-flash\n'
fi
# Only the private startup file needs the key; web and worker processes do not.
unset OPENROUTER_KEY OPENROUTER_API_KEY
if [[ ! -x node_modules/.bin/concurrently ]]; then
    npm ci
fi

packages=(-p moor-daemon -p moor-web-host)
if [[ $telnet == true ]]; then packages+=(-p moor-telnet-host); fi
if [[ $curl_worker == true ]]; then packages+=(-p moor-curl-worker); fi
if [[ $git_worker == true ]]; then packages+=(-p moor-git-worker); fi
build_flags=()
if [[ $profile == release ]]; then build_flags+=(--release); fi

# Build together rather than having several cargo run processes contend for its lock.
cargo build "${packages[@]}" "${build_flags[@]}" --bins
target_dir=$(cargo metadata --no-deps --format-version 1 | node -e '
    let input = "";
    process.stdin.on("data", chunk => input += chunk);
    process.stdin.on("end", () => process.stdout.write(JSON.parse(input).target_directory));
')
bin_dir=$target_dir/$profile
npm run web:prepare

mkdir -p -- "$data_dir"
data_dir=$(cd -- "$data_dir" && pwd -P)
if [[ $clean == true ]]; then
    core_dir=$(cd -- "$core" && pwd -P)
    if [[ $data_dir == / || "$PWD/" == "$data_dir/"* || "$core_dir/" == "$data_dir/"* ]]; then
        echo "Refusing to wipe a data directory containing the repository or core: $data_dir" >&2
        exit 2
    fi
    printf 'Wiping data directory: %s\n' "$data_dir"
    rm -rf -- "$data_dir"
    mkdir -p -- "$data_dir"
fi
ipc_dir=$data_dir/ipc
key_dir=$data_dir/keys
(umask 077; mkdir -p -- "$ipc_dir" "$key_dir")
rpc_address=ipc://$ipc_dir/rpc.sock
events_address=ipc://$ipc_dir/events.sock
requests_address=ipc://$ipc_dir/workers-request.sock
responses_address=ipc://$ipc_dir/workers-response.sock
enrollment_address=ipc://$ipc_dir/enrollment.sock

client_args=(
    --rpc-address "$rpc_address"
    --events-address "$events_address"
    --workers-request-address "$requests_address"
    --workers-response-address "$responses_address"
    --enrollment-address "$enrollment_address"
)
daemon_args=(
    "$data_dir" --db "${MOOR_DB:-development.db}"
    --import "$core" --import-format objdef
    --export "${MOOR_EXPORT:-development-export}" --export-format objdef
    --rpc-listen "$rpc_address" --events-listen "$events_address"
    --workers-request-listen "$requests_address"
    --workers-response-listen "$responses_address"
    --enrollment-listen "$enrollment_address"
    --generate-keypair
    --private-key "$key_dir/signing.pem" --public-key "$key_dir/verifying.pem"
    --enrollment-token-file "$key_dir/enrollment-token"
    --rich-notify true --type-dispatch true --flyweight-type true
    --bool-type true --symbol-type true --use-boolean-returns true
    --custom-errors true --use-uuobjids true --anonymous-objects true
    --enable-eventlog true --use-symbols-in-builtins true
)

if [[ -n $git_upstream ]]; then daemon_args+=(--git-upstream "$git_upstream"); fi
if [[ -n $baseline_objdef_dir ]]; then daemon_args+=(--baseline-objdef-dir "$baseline_objdef_dir"); fi

# concurrently uses a shell. Quote each argument for the explicitly selected Bash shell.
commands=()
names=(daemon web vite)
add_command() {
    local command
    printf -v command '%q ' "$@"
    commands+=("exec $command")
}
add_command "$bin_dir/moor-daemon" "${daemon_args[@]}"
if [[ -n $startup_config_dir ]]; then
    # The launcher execs concurrently below, so its EXIT trap cannot remove the key. The daemon
    # reads the key through a pipe, and the file is removed once the pipe has the contents.
    printf -v startup_source '<(cat -- %q && rm -rf -- %q)' \
        "$startup_config_dir/config.json" "$startup_config_dir"
    commands[-1]+="--startup-config $startup_source"
fi
add_command "$bin_dir/moor-web-host" "${client_args[@]}" \
    --data-dir "$data_dir/web-host" --listen-address 0.0.0.0:8080 --enable-webhooks
add_command npm run dev --workspace meadow -- --host 0.0.0.0 --strictPort
if [[ $telnet == true ]]; then
    names+=(telnet)
    add_command "$bin_dir/moor-telnet-host" "${client_args[@]}" \
        --data-dir "$data_dir/telnet-host" --telnet-address 0.0.0.0 --telnet-port 8888
fi
if [[ $curl_worker == true ]]; then
    names+=(curl)
    add_command "$bin_dir/moor-curl-worker" "${client_args[@]}" --data-dir "$data_dir/curl-worker"
fi
if [[ $git_worker == true ]]; then
    names+=(git)
    add_command "$bin_dir/moor-git-worker" "${client_args[@]}" \
        --data-dir "$data_dir/git-worker" --work-dir "$data_dir/git-jobs"
fi

export RUST_BACKTRACE=${RUST_BACKTRACE:-1}
printf 'Core: %s\nData: %s\nMeadow: http://localhost:3000\nWeb host: http://localhost:8080\n' "$core" "$data_dir"
printf 'Both web servers listen on all IPv4 interfaces; remote access: http://<this-machine-IP>:3000\n'
if [[ $telnet == true ]]; then
    printf 'Telnet: telnet localhost 8888 (listening on all IPv4 interfaces)\n'
fi
printf 'Worker IPC:\n  RPC: %s\n  Requests: %s\n  Responses: %s\n' "$rpc_address" "$requests_address" "$responses_address"
service_names=$(IFS=,; echo "${names[*]}")
exec node_modules/.bin/concurrently --shell "$(command -v bash)" \
    --names "$service_names" --kill-others --kill-signal SIGINT --kill-timeout 30000 \
    "${commands[@]}"

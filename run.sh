#!/bin/bash -e

# Cargo watch ignores
watch_ignores="{benchmarking,logs}"

# Change to right directory.
folder="$(dirname "$0")"
cd "$folder"

local_run=run.local.sh

watch=0
flame=0
release=""
while true; do case "$1" in
    w|watch)
        watch=1
        shift
        ;;
    f|flame)
        flame=1
        shift
        ;;
    r|release)
        release="--release"
        shift
        ;;
    *)
        break;
        ;;
esac done

bin="$1"
shift

rust_log="$RUST_LOG"

append_rust_log() {
    if [ ! -z "$rust_log" ]; then
        rust_log="$rust_log,"
    fi
    rust_log="$rust_log$1"
}

level() { append_rust_log "$1"; }

trace() { append_rust_log "$1=trace"; }
debug() { append_rust_log "$1=debug"; }
info()  { append_rust_log "$1=info"; }
warn()  { append_rust_log "$1=warn"; }
error() { append_rust_log "$1=error"; }
off()   { append_rust_log "$1=off"; }

if [ -f "$local_run" ]; then
    source "$local_run"
fi

export RUST_LOG="$rust_log"

echo $watch_ignores
if [ "$flame" == "1" ]; then
    if [ "$(uname -s)" == "Darwin" ]; then
        cargo build --release --bin "$bin"
        cmd="target/release/$bin"
        for c in "$@"; do
            cmd="$cmd $c"
        done
        exec sudo -E flamegraph "$cmd"
    fi

    exec cargo flamegraph --bin "$bin" -- "$@"
elif [ "$watch" == "1" ]; then
    cmd="run $release --bin '$bin' --"
    for c in "$@"; do
        cmd="$cmd '$c'"
    done
    cargo watch -i $watch_ignores -x "$cmd"
else
    exec cargo run $release --bin "$bin" -- "$@"
fi

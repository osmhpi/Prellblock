#!/bin/sh -e

local_run=${0%.sh}.local.sh

rust_log="$RUST_LOG"

append_rust_log() {
    if [ ! -z "$rust_log" ]; then
        rust_log="$rust_log,"
    fi
    rust_log="$rust_log$1"
}

level() {
    append_rust_log "$1"
}

set_rust_log() {
    append_rust_log "$2=$1"
}

alias trace="set_rust_log trace"
alias debug="set_rust_log debug"
alias info="set_rust_log info"
alias warn="set_rust_log warn"
alias error="set_rust_log error"
alias off="set_rust_log off"

if [ -f "$local_run" ]; then
    source "$local_run"
fi

export RUST_LOG="$rust_log"

exec cargo run "$@"

#!/bin/bash

folder="$(dirname $0)"

pkill prellblock

pids=()
start_peers() {
    # Remove old data.
    rm -r "$folder/../data"
    sleep 1
    export RUST_BACKTRACE=1
    cargo run --bin prellblock --release -- emily 2>/dev/null & emily_pid=$!
    cargo run --bin prellblock --release -- james 2>/dev/null & james_pid=$!
    cargo run --bin prellblock --release -- percy 2>/dev/null & percy_pid=$!
    cargo run --bin prellblock --release -- thomas 2>/dev/null & thomas_pid=$!
    pids=($emily_pid $james_pid $percy_pid $thomas_pid)
    sleep 3
}

# Bytes of message size.
sizes=(8 16 32 64 256 512 1024 2048)
# Number of runs for each size.
runs=5
# Number of transactions to send for each run.
txs=5000

filename="$folder/tps-$(date "+%Y-%m-%d-%H-%M-%S.dat")"
echo "serde_json" >> $filename
for size in ${sizes[@]}; do
    start_peers

    run_tps=()
    for (( run=1; run<=$runs; run++ )); do
        echo "Running benchmark with $txs transactions and message size $size ($run of $runs)"
        tps=$(cargo run --bin prellblock-client --release -- bench emily benchmark $txs --print-minimal -s $size 2>/dev/null)
        echo "TPS: $tps"
        run_tps+=( $tps )
    done;

    vals=$(( IFS=$'\n'; echo "${run_tps[*]}" ) | awk '{if(min==""){min=max=$1}; if($1>max) {max=$1}; if($1<min) {min=$1}; total+=$1; count+=1} END {printf "%s,%s,%s", total/count, min, max}')

    echo "$size,$vals" >> $filename
    
    kill "${pids[@]}"
    sleep 2
    echo "--------------------------------------------------------------------------------"
done

gnuplot -e "datafile='$filename'; outfile='$folder/tps.png'; load '$folder/tps.plt'"

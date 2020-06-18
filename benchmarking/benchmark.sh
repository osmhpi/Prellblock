#!/bin/bash

folder="$(dirname $0)"

pids=()
start_peers() {
    # Remove old data.
    docker-compose down -t0
    sudo rm -rf "$folder/../data" "$folder/../blocks"
    sleep 1
    docker-compose up -d
    sleep 5
}

# Bytes of message size.
sizes=(1 2 4 8 16 32 64 256 512 1024 2048)
# Number of runs for each size.
runs=5
# Number of transactions to send for each run.
txs=20000

filename="$folder/tps-$(date "+%Y-%m-%d-%H-%M-%S.dat")"
echo "Prellblock" >> $filename
for size in ${sizes[@]}; do
    start_peers

    run_tps=()
    for (( run=1; run<=$runs; run++ )); do
        echo "Running benchmark with $txs transactions and message size $size ($run of $runs)"
        tps=$(cargo run --bin prellblock-client --release -- bench alice benchmark $txs -s $size --print-tps 2>/dev/null)
        echo "TPS: $tps"
        run_tps+=( $tps )
    done;

    vals=$(( IFS=$'\n'; echo "${run_tps[*]}" ) | awk '{if(min==""){min=max=$1}; if($1>max) {max=$1}; if($1<min) {min=$1}; total+=$1; count+=1} END {printf "%s,%s,%s", total/count, min, max}')

    echo "$size,$vals" >> $filename
    
    docker-compose down -t0
    sleep 2
    echo "--------------------------------------------------------------------------------"
done

gnuplot -e "datafile='$filename'; outfile='$folder/tps.png'; load '$folder/tps.plt'"

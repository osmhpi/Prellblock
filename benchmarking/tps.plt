set title 'Prellblock - Transactions Per Second'

set key autotitle columnhead
set ylabel "TPS" 
set xlabel "Size Of Message (Bytes)"
set yrange [0:*]
set xrange [1:5000]
set logscale x

# Output png
set terminal png
set output outfile

set datafile separator ','

plot datafile with lines linetype 1, datafile using 1:2:3:4 with yerrorbars linetype 1
set title 'Transaktionen pro Sekunde'

set key autotitle columnhead
set ylabel "TPS" 
set xlabel "Nutzlastgröße der Transaktion (Bytes)"
set yrange [0:*]
set xrange [1:5000]
set logscale x

# Output png
set terminal png
set output outfile

set datafile separator ','

plot datafile with lines linetype 1 notitle, datafile using 1:2:3:4 with yerrorbars linetype 1
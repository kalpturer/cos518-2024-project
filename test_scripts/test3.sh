#!/bin/bash

experiment_len=30

echo "Starting 3 replicas"

# Start replica 1
./target/release/project --id 1 -n 3 --listener 127.0.0.1:6000 --connections 127.0.0.1:8000 127.0.0.1:9000 > replica1.log 2> replica1.err &
# Get its process ID
PID1=$!
# Sleep for 1 second
sleep 1

# Start replica 2
./target/release/project --id 2 -n 3 --listener 127.0.0.1:8000 --connections 127.0.0.1:6000 127.0.0.1:9000 > replica2.log 2> replica2.err &
# Get its process ID
PID2=$!
# Sleep for 1 second
sleep 1

# Start replica 3
./target/release/project --id 3 -n 3 --listener 127.0.0.1:9000 --connections 127.0.0.1:6000 127.0.0.1:8000 > replica3.log 2> replica3.err &
# Get its process ID
PID3=$!
# Sleep for 5 seconds
sleep 10

echo "Finished starting 3 replicas"


echo "Starting 3 client request generators"
# Start 3 clients, one for each replica that generate random requests
./target/release/project --gen 127.0.0.1:6000 --time-sleep 20 --experiment-time $experiment_len --listener 127.0.0.1:6001 > client1.log 2>&1 &
PID4=$!
#sleep 5

./target/release/project --gen 127.0.0.1:8000 --time-sleep 20 --experiment-time $experiment_len --listener 127.0.0.1:8001 > client2.log 2>&1 &
PID5=$!
#sleep 5

./target/release/project --gen 127.0.0.1:9000 --time-sleep 20 --experiment-time $experiment_len --listener 127.0.0.1:9001 > client3.log 2>&1 &
PID6=$!
#sleep 5
echo "Finished starting 3 client request generators"

echo "Sleeping for _ seconds"
# Sleep for 30 seconds

sleep $experiment_len
echo "Finished sleeping"

sleep 10


echo "Saving replica states"
# Fetch the longest chain transactions from all three nodes and write the output to files
./target/release/project --save 127.0.0.1:6000 > /dev/null 2>&1 &
PID7=$!
./target/release/project --save 127.0.0.1:8000 > /dev/null 2>&1 &
PID8=$!
./target/release/project --save 127.0.0.1:9000 > /dev/null 2>&1 &
PID9=$!
echo "Finished saving replica states"

sleep 10

echo "Killing processes"
# Kill the target/release/project node processes
kill $PID1 $PID2 $PID3 


# Output that the process is completed
echo "Test complete, outputs saved"
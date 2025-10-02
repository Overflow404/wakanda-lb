#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <max_request_count> <url>"
    exit 1
fi

if [ -z "$2" ]; then
    echo "Usage: $0 <max_request_count> <url>"
    exit 1
fi

MAX_REQUESTS=$1
URL=$2

echo "Executing benchmark with ${MAX_REQUESTS} requests towards ${URL}"

results=$(curl --parallel --parallel-immediate --parallel-max 50 \
     -o /dev/null -w "%{http_code} %{time_total}\n" \
     $(yes "$URL" | head -n $MAX_REQUESTS))

if echo "$results" | awk '{print $1}' | grep -vq '^200$'; then
    echo "Some URLs failed"
else
    echo "All URLs returned 200"
fi

avg=$(echo "$results" | awk '{sum += $2; count++} END {if(count>0) printf "%.3f", sum/count}')
echo "Average response time: ${avg}s"

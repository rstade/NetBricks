#!/usr/bin/env bash

#!/bin/bash

set -e

if [ $# -ge 1 ]; then
    TASK=$1
else
    TASK=all
fi

case $TASK in

    all)
        export RUST_LOG="e2d2=info", RUST_BACKTRACE=1
        cargo test $2 --no-run --message-format=json  > out.txt
        executable=`cat out.txt | jq -r 'select((.profile.test == true) and (.target.name == "e2d2")) | .filenames[]'`
        echo $executable
        sudo -E env "PATH=$PATH" $executable --nocapture
        executable=`cat out.txt | jq -r 'select((.profile.test == true) and (.target.name == "address")) | .filenames[]'`
        echo $executable
        sudo -E env "PATH=$PATH" $executable --nocapture
        executable=`cat out.txt | jq -r 'select((.profile.test == true) and (.target.name == "ring_buffer")) | .filenames[]'`
        echo $executable
        sudo -E env "PATH=$PATH" $executable --nocapture
        executable=`cat out.txt | jq -r 'select((.profile.test == true) and (.target.name == "tcp_window")) | .filenames[]'`
        echo $executable
        sudo -E env "PATH=$PATH" $executable --nocapture
        ;;



esac




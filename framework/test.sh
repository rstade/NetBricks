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
        executable=`cargo test $2 --no-run --message-format=json  | jq -r 'select((.profile.test == true) and (.target.name == "e2d2")) | .filenames[]'`
        echo $executable
        sudo -E env "PATH=$PATH" $executable --nocapture
        ;;



esac




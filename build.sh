#!/bin/bash
# Stop on any errors
set -e

BASE_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd)"
BUILD_SCRIPT=$( basename "$0" )

echo "BASE_DIR=" $BASE_DIR
DPDK_VER=20.11
DPDK_LD_PATH="/usr/local/lib64"


CARGO_LOC=`which cargo || true`
export CARGO=${CARGO_PATH-"${CARGO_LOC}"}
if [ -z ${CARGO} ] || [ ! -e ${CARGO} ]; then
    echo "Could not find a preinstalled Cargo in PATH. Set CARGO_PATH if necessary."
    exit 1
fi
echo "Using Cargo from ${CARGO}"

NATIVE_LIB_PATH="${BASE_DIR}/native"
export SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt

source ${BASE_DIR}/examples.sh
REQUIRE_RUSTFMT=0
export RUSTFLAGS="-C target-cpu=native"

native () {
    make -j $proc -C $BASE_DIR/native
#    make -C $BASE_DIR/native install
}


print_examples () {
    echo "The following examples are available:"
    for eg in ${examples[@]}; do
        if [ -e ${BASE_DIR}/${eg}/Cargo.toml ]; then
            target=$( ${CARGO} read-manifest --manifest-path ${BASE_DIR}/${eg}/Cargo.toml | ${BASE_DIR}/scripts/read-target.py - )
            printf "\t %s\n" ${target}
        fi
    done
    exit 0
}

clean () {
    pushd $BASE_DIR/framework
    ${CARGO} clean || true
    popd

    pushd $BASE_DIR/test/framework
    ${CARGO} clean || true
    popd

    for example in ${examples[@]}; do
        pushd ${BASE_DIR}/$example
        ${CARGO} clean || true
        popd
    done
    make clean -C ${BASE_DIR}/native
    rm -rf ${BASE_DIR}/target 
}

UNWIND_BUILD="${TOOLS_BASE}"/libunwind

clean_deps() {
    echo "Cleaning dependencies"
    rm -rf ${BIN_DIR} || true
    rm -rf ${DOWNLOAD_DIR} || true
    rm -rf ${TOOLS_BASE} || true
    rm -rf ${LLVM_RESULT} || true
    rm -rf ${MUSL_RESULT} || true
    rm -rf ${DPDK_HOME} || true
    echo "Cleaned DEPS"
}

if [ $# -ge 1 ]; then
    TASK=$1
else
    TASK=build
fi

case $TASK in
    build_test)
        shift
        if [ $# -lt 1 ]; then
            echo Can build one of the following tests:
            for example in ${examples[@]}; do
                base_eg=$( basename ${example} )
                printf "\t %s\n" ${base_eg}
            done
            exit 1
        fi
        build_dir=$1
        if [ ! -e ${BASE_DIR}/test/${build_dir}/Cargo.toml ]; then
            echo "No Cargo.toml, not valid"
        fi
        pushd ${BASE_DIR}/test/${build_dir}
            ${CARGO} build --release
        popd
        ;;
    build_fmwk)
        native
        pushd $BASE_DIR/framework
        ${CARGO} build --release
        popd
        ;;
    build)
        native
        pushd $BASE_DIR/framework
        ${CARGO} build --release
        popd

        for example in ${examples[@]}; do
            pushd ${BASE_DIR}/${example}
            ${CARGO} build --release
            popd
        done
        ;;
    build_debug)
        native
        pushd $BASE_DIR/framework
        ${CARGO} build
        popd

        for example in ${examples[@]}; do
            pushd ${BASE_DIR}/${example}
            ${CARGO} build
            popd
        done
        ;;
    huge_pages)
	./hugepages.sh
//        sudo dpdk-hugepages.py -p 2M --setup 8G
//        sudo dpdk-hugepages.py -s
        ;;
    test)
        native
        ./hugepages.sh
        pushd $BASE_DIR/framework
        export LD_LIBRARY_PATH="${NATIVE_LIB_PATH}:${DPDK_LD_PATH}:${TOOLS_BASE}:${LD_LIBRARY_PATH}"
#        sudo -E env "PATH=$PATH" ${CARGO} test --release
        ${CARGO} test --release -- $2
        popd

        for testname in tcp_payload macswap; do
          pushd $BASE_DIR/test/$testname
          ./check.sh
          popd
        done
        ;;
    unittest)
        pushd $BASE_DIR/framework
        ./test.sh all --release
        popd
        ;;
    run)
        shift
        if [ $# -le 0 ]; then
            print_examples
        fi
        cmd=$1
        shift
        executable=${BASE_DIR}/target/release/$cmd
        if [ ! -e ${executable} ]; then
            echo "${executable} not found, building"
            ${BASE_DIR}/${BUILD_SCRIPT} build
        fi
        export PATH="${BIN_DIR}:${PATH}"
        export LD_LIBRARY_PATH="${NATIVE_LIB_PATH}:${DPDK_LD_PATH}:${TOOLS_BASE}:${LD_LIBRARY_PATH}"
        sudo env PATH="$PATH" LD_LIBRARY_PATH="$LD_LIBRARY_PATH" LD_PRELOAD="$LD_PRELOAD" \
            $executable "$@"
        ;;
    debug)
        shift
        if [ $# -le 0 ]; then
            print_examples
        fi
        cmd=$1
        shift
        executable=${BASE_DIR}/target/release/$cmd
        if [ ! -e ${executable} ]; then
            echo "${executable} not found, building"
            ${BASE_DIR}/${BUILD_SCRIPT} build
        fi
        export PATH="${BIN_DIR}:${PATH}"
        export LD_LIBRARY_PATH="${NATIVE_LIB_PATH}:${DPDK_LD_PATH}:${TOOLS_BASE}:${LD_LIBRARY_PATH}"
        sudo env PATH="$PATH" LD_LIBRARY_PATH="$LD_LIBRARY_PATH" LD_PRELOAD="$LD_PRELOAD" \
            rust-gdb --args $executable "$@"
        ;;
    check_examples)
        python3 scripts/check-examples.py "${examples[@]}"
        ;;
    dist_clean)
        clean
        ;;
    clean)
        clean
        ;;
    env)
        echo "export PATH=\"${BIN_DIR}:${PATH}\""
        echo "export LD_LIBRARY_PATH=\"${NATIVE_LIB_PATH}:${TOOLS_BASE}:${LD_LIBRARY_PATH}\""
        ;;
    *)
        cat <<endhelp
./build.sh <Command>
      Where command is one of
          deps: Build dependencies
          build: Build the project (this includes framework and all tests).
          build_fmwk: Just build framework.
          build_test: Build a particular test.
          test: Run unit tests.
          run: Run one of the examples (Must specify example name and arguments).
          debug: Debug one of the examples (Must specify example name and examples).
          doc: Run rustdoc and produce documentation
          clean: Remove all built files
          dist_clean: Remove all support files
          env: Environment variables, run as eval \`./build.sh env\`.
          huge_pages: setup huge pages for dpdk
endhelp
        ;;
esac

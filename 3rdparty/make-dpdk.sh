#!/bin/bash
# Stop on any errors
set -e
BASE_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd)"
DOWNLOAD_BASE="${1-$BASE_DIR}"
echo Using "$DOWNLOAD_BASE" for downloads
DPDK_VER=${DPDK_VER-"17.08"}
MODE=download # or git
DOWNLOAD_PATH="${DOWNLOAD_BASE}/dpdk.tar.xz"
DPDK_RESULT="${BASE_DIR}/dpdk"
CONFIG_FILE=${DPDK_CONFIG_FILE-"${BASE_DIR}/dpdk-confs/common_linuxapp-${DPDK_VER}"}
CONFIG_PFX=${DPDK_CONFIG_PFX-""}
echo "Using configuration ${CONFIG_FILE}${CONFIG_PFX}"

cp "${CONFIG_FILE}${CONFIG_PFX}" "${DPDK_RESULT}/config/common_linuxapp"
export RTE_TARGET=x86_64-native-linuxapp-gcc
FLAGS="-g3 -Wno-error=maybe-uninitialized -fPIC"
make config -C "${DPDK_RESULT}" T=x86_64-native-linuxapp-gcc \
	EXTRA_CFLAGS="$FLAGS"
PROCS="$(nproc)"
make -j $PROCS -C "${DPDK_RESULT}" EXTRA_CFLAGS="$FLAGS"

#!/bin/bash
cp rte_mbuf_api.rs rte_mbuf_api.rs.bak
cp rte_mbuf_core_api.rs rte_mbuf_core_api.rs.bak
cp rte_ethdev_api.rs rte_ethdev_api.rs.bak
bindgen --no-layout-tests rte_mbuf.h -o rte_mbuf_api.rs
bindgen --no-layout-tests rte_ethdev.h -o rte_ethdev_api.rs
bindgen --no-layout-tests rte_mbuf_core.h -o rte_mbuf_core_api.rs
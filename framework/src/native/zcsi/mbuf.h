//
// Created by rainer on 11.12.18.
// addapted to DPDK 18.11, 04.03.2019
//

#include <stdint.h>

__extension__
struct L234len {
    uint64_t l2_len:7;
    uint64_t l3_len:9;
    uint64_t l4_len:8;
    uint64_t tso_segsz:16;
    /* fields for TX offloading of tunnels */
    uint64_t outer_l3_len:9;
    uint64_t outer_l2_len:7;
    /* uint64_t unused:8; */
};

union TxOffload {
    uint64_t tx_offload;
    struct L234len l234len;
};

struct MBuf {
    uint8_t* buf_addr;
    uint64_t phys_addr;
    uint16_t data_off;
    uint16_t refcnt;
    uint16_t nb_segs;
    uint16_t port;
    uint64_t ol_flags;
    uint32_t packet_type;
    uint32_t pkt_len;
    uint16_t data_len;
    uint16_t vlan_tci;
    uint32_t hash_rss;
    uint32_t hash_hi;
    uint16_t vlan_tci_outer;
    uint16_t buf_len;        //  /**< Length of segment buffer. */
    uint64_t timestamp;      // new
    // here starts the second cacheline
    uint64_t userdata;
    uint64_t pool;
    struct MBuf* next;
    union TxOffload tx_offload;
    uint16_t priv_size;
    uint16_t timesync;
    uint32_t seqn; // /** Sequence number. See also rte_reorder_insert(). */
    struct rte_mbuf_ext_shared_info *shinfo;
};

typedef void (*rte_mbuf_extbuf_free_callback_t)(void *addr, void *opaque);
typedef struct {
        volatile int16_t cnt;
} rte_atomic16_t;

struct rte_mbuf_ext_shared_info {
    rte_mbuf_extbuf_free_callback_t free_cb;
    void *fcb_opaque;
    rte_atomic16_t refcnt_atomic;
};
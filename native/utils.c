#include <rte_config.h>
#include <rte_hash_crc.h>
#include <rte_ip.h>

// Make rte_hash_crc available to Rust. This adds some cost, will look into producing a pure Rust
// version.
uint32_t crc_hash_native(const void* data, uint32_t len, uint32_t initial) {
    return rte_hash_crc(data, len, initial);
}

uint16_t ipv4_cksum(const void* iphdr) {
    return rte_ipv4_cksum((const struct rte_ipv4_hdr*)iphdr);
}

uint16_t ipv4_phdr_chksum(const struct rte_ipv4_hdr* ipv4_hdr, uint64_t ol_flags)
{
    return rte_ipv4_phdr_cksum(ipv4_hdr, ol_flags);
}

int validate_tx_offload(const struct rte_mbuf *m)
{
    return rte_validate_tx_offload(m);
}
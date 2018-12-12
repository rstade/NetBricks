/*
   50  * Packet Offload Features Flags. It also carry packet type information.
   51  * Critical resources. Both rx/tx shared these bits. Be cautious on any change
   52  *
   53  * - RX flags start at bit position zero, and get added to the left of previous
   54  *   flags.
   55  * - The most-significant 3 bits are reserved for generic mbuf flags
   56  * - TX flags therefore start at bit position 60 (i.e. 63-3), and new flags get
   57  *   added to the right of the previously defined flags i.e. they should count
   58  *   downwards, not upwards.
   59  *
   60  * Keep these flags synchronized with rte_get_rx_ol_flag_name() and
   61  * rte_get_tx_ol_flag_name().
   62
*/
#define PKT_RX_VLAN          (1ULL << 0)

#define PKT_RX_RSS_HASH      (1ULL << 1)
#define PKT_RX_FDIR          (1ULL << 2)
#define PKT_RX_L4_CKSUM_BAD  (1ULL << 3)

#define PKT_RX_IP_CKSUM_BAD  (1ULL << 4)

#define PKT_RX_EIP_CKSUM_BAD (1ULL << 5)
#define PKT_RX_VLAN_STRIPPED (1ULL << 6)

#define PKT_RX_IP_CKSUM_MASK ((1ULL << 4) | (1ULL << 7))

#define PKT_RX_IP_CKSUM_UNKNOWN 0
#define PKT_RX_IP_CKSUM_BAD     (1ULL << 4)
#define PKT_RX_IP_CKSUM_GOOD    (1ULL << 7)
#define PKT_RX_IP_CKSUM_NONE    ((1ULL << 4) | (1ULL << 7))

#define PKT_RX_L4_CKSUM_MASK ((1ULL << 3) | (1ULL << 8))

#define PKT_RX_L4_CKSUM_UNKNOWN 0
#define PKT_RX_L4_CKSUM_BAD     (1ULL << 3)
#define PKT_RX_L4_CKSUM_GOOD    (1ULL << 8)
#define PKT_RX_L4_CKSUM_NONE    ((1ULL << 3) | (1ULL << 8))

#define PKT_RX_IEEE1588_PTP  (1ULL << 9)
#define PKT_RX_IEEE1588_TMST (1ULL << 10)
#define PKT_RX_FDIR_ID       (1ULL << 13)
#define PKT_RX_FDIR_FLX      (1ULL << 14)
#define PKT_RX_QINQ_STRIPPED (1ULL << 15)

#define PKT_RX_LRO           (1ULL << 16)

#define PKT_RX_TIMESTAMP     (1ULL << 17)

#define PKT_RX_SEC_OFFLOAD              (1ULL << 18)

#define PKT_RX_SEC_OFFLOAD_FAILED       (1ULL << 19)

#define PKT_RX_QINQ          (1ULL << 20)

/* add new RX flags here */

/* add new TX flags here */

#define PKT_TX_UDP_SEG  (1ULL << 42)

#define PKT_TX_SEC_OFFLOAD              (1ULL << 43)

#define PKT_TX_MACSEC        (1ULL << 44)

#define PKT_TX_TUNNEL_VXLAN   (0x1ULL << 45)
#define PKT_TX_TUNNEL_GRE     (0x2ULL << 45)
#define PKT_TX_TUNNEL_IPIP    (0x3ULL << 45)
#define PKT_TX_TUNNEL_GENEVE  (0x4ULL << 45)

#define PKT_TX_TUNNEL_MPLSINUDP (0x5ULL << 45)
/* add new TX TUNNEL type here */
#define PKT_TX_TUNNEL_MASK    (0xFULL << 45)

#define PKT_TX_QINQ        (1ULL << 49)
/* this old name is deprecated */
#define PKT_TX_QINQ_PKT    PKT_TX_QINQ

#define PKT_TX_TCP_SEG       (1ULL << 50)

#define PKT_TX_IEEE1588_TMST (1ULL << 51)
#define PKT_TX_L4_NO_CKSUM   (0ULL << 52)
#define PKT_TX_TCP_CKSUM     (1ULL << 52)
#define PKT_TX_SCTP_CKSUM    (2ULL << 52)
#define PKT_TX_UDP_CKSUM     (3ULL << 52)
#define PKT_TX_L4_MASK       (3ULL << 52)
#define PKT_TX_IP_CKSUM      (1ULL << 54)

#define PKT_TX_IPV4          (1ULL << 55)

#define PKT_TX_IPV6          (1ULL << 56)

#define PKT_TX_VLAN          (1ULL << 57)
/* this old name is deprecated */
#define PKT_TX_VLAN_PKT      PKT_TX_VLAN

#define PKT_TX_OUTER_IP_CKSUM   (1ULL << 58)

#define PKT_TX_OUTER_IPV4   (1ULL << 59)

#define PKT_TX_OUTER_IPV6    (1ULL << 60)

#define PKT_TX_OFFLOAD_MASK (    \
                PKT_TX_IP_CKSUM |        \
                PKT_TX_L4_MASK |         \
                PKT_TX_OUTER_IP_CKSUM |  \
                PKT_TX_TCP_SEG |         \
                PKT_TX_IEEE1588_TMST |   \
                PKT_TX_QINQ_PKT |        \
                PKT_TX_VLAN_PKT |        \
                PKT_TX_TUNNEL_MASK |     \
                PKT_TX_MACSEC |          \
                PKT_TX_SEC_OFFLOAD)

#define __RESERVED           (1ULL << 61)
#define IND_ATTACHED_MBUF    (1ULL << 62)
/* Use final bit of flags to indicate a control mbuf */
#define CTRL_MBUF_FLAG       (1ULL << 63)

#include <rte_version.h>
#include <rte_bus_pci.h>
#include <rte_config.h>
#include <rte_pci.h>
#include <rte_eal.h>
#include <rte_ethdev.h>
#include "mempool.h"
#include "fdir.h"

/*
 * RX and TX Prefetch, Host, and Write-back threshold values should be
 * carefully set for optimal performance. Consult the network
 * controller's datasheet and supporting DPDK documentation for guidance
 * on how these parameters should be set.
 */
#define RX_PTHRESH 			8 /**< Default values of RX prefetch threshold reg. */
#define RX_HTHRESH 			8 /**< Default values of RX host threshold reg. */
#define RX_WTHRESH 			0 /**< Default values of RX write-back threshold reg. */
#define RX_FREE_THRESH     32

/*
 * These default values are optimized for use with the Intel(R) 82599 10 GbE
 * Controller and the DPDK ixgbe PMD. Consider using other values for other
 * network controllers and/or network drivers.
 */
#define TX_PTHRESH 			36 /**< Default values of TX prefetch threshold reg. */
#define TX_HTHRESH			0  /**< Default values of TX host threshold reg. */
#define TX_WTHRESH			0  /**< Default values of TX write-back threshold reg. */


#define HW_RXCSUM 0
#define HW_TXCSUM 0
#define MIN(a, b) ((a) < (b) ? (a) : (b))
static const struct rte_eth_conf default_eth_conf = {
    .link_speeds = ETH_LINK_SPEED_AUTONEG, /* auto negotiate speed */
    /*.link_duplex = ETH_LINK_AUTONEG_DUPLEX,	[> auto negotiation duplex <]*/
    .lpbk_mode = 0, /* Loopback operation mode. By default the value is 0, meaning the loopback mode is disabled. */
    .rxmode =
        {
            .mq_mode        = ETH_MQ_RX_RSS, /* Use RSS without DCB or VMDQ */
            .max_rx_pkt_len = 0,             /* valid only if jumbo is on */
            .split_hdr_size = 0,             /* valid only if HS is on */
            .header_split   = 0,             /* Header Split off */
            .hw_ip_checksum = HW_RXCSUM,     /* IP checksum offload */
            .hw_vlan_filter = 0,             /* VLAN filtering */
            .hw_vlan_strip  = 0,             /* VLAN strip */
            .hw_vlan_extend = 0,             /* Extended VLAN */
            .jumbo_frame    = 0,             /* Jumbo Frame support */
            .hw_strip_crc   = 1,             /* CRC stripped by hardware */
			.enable_scatter = 0,			 /* Enable scatter packets rx handler */
			.enable_lro     = 0,			 /* Enable LRO */
        },
    .txmode =
        {
            .mq_mode = ETH_MQ_TX_NONE, /* Disable DCB and VMDQ */
			.hw_vlan_reject_tagged = 0,
			.hw_vlan_reject_untagged = 0,
			.hw_vlan_insert_pvid = 0,
        },
    /* is later on re-written taking the info from rte_eth_dev_get_info */
    .rx_adv_conf.rss_conf =
        {
            .rss_hf = ETH_RSS_IP | ETH_RSS_UDP | ETH_RSS_TCP | ETH_RSS_SCTP, .rss_key = NULL,
        },
    /* we need the flow director feature*/
    .fdir_conf =
        {
            .mode = RTE_FDIR_MODE_PERFECT,
            .pballoc = RTE_FDIR_PBALLOC_256K,
            .status = RTE_FDIR_NO_REPORT_STATUS,
            .drop_queue = 0,
            .mask = { // every mask in big endian
                .vlan_tci_mask=0,
                .ipv4_mask= {
                    .src_ip =0,
                    .dst_ip =0xFFFFFFFF,
                    .tos    =0,
                    .ttl    =0,
                    .proto  =0,
                },
                .ipv6_mask= {
                },
                .src_port_mask=0x0000,
                .dst_port_mask=0x00FC,
                .mac_addr_byte_mask=0,
                .tunnel_id_mask=0,
                .tunnel_type_mask=0,
            },
            .flex_conf =  {
                    .nb_payloads = 0,
                    .nb_flexmasks = 0,
            }
        },
    /* No interrupt */
    .intr_conf =
        {
            .lsc = 0,
        },
};

int num_pmd_ports() {
    return rte_eth_dev_count();
}

int get_pmd_ports(struct rte_eth_dev_info* info, int len) {
    int num_ports   = rte_eth_dev_count();
    int num_entries = MIN(num_ports, len);
    for (int i = 0; i < num_entries; i++) {
        memset(&info[i], 0, sizeof(struct rte_eth_dev_info));
        rte_eth_dev_info_get(i, &info[i]);
    }
    return num_entries;
}

int get_rte_eth_dev_info(int dev, struct rte_eth_dev_info* info) {
    if (dev >= rte_eth_dev_count()) {
        return -ENODEV;
    } else {
        rte_eth_dev_info_get(dev, info);
        return 0;
    }
}

int max_rxqs(int dev) {
    struct rte_eth_dev_info info;
    if (get_rte_eth_dev_info(dev, &info) != 0) {
        return -ENODEV;
    } else {
        return info.max_rx_queues;
    }
}

int max_txqs(int dev) {
    struct rte_eth_dev_info info;
    if (get_rte_eth_dev_info(dev, &info) != 0) {
        return -ENODEV;
    } else {
        return info.max_tx_queues;
    }
}

void enumerate_pmd_ports() {
    int num_dpdk_ports = rte_eth_dev_count();
    int i;

    printf("%d DPDK PMD ports have been recognized:\n", num_dpdk_ports);
    for (i = 0; i < num_dpdk_ports; i++) {
        struct rte_eth_dev_info dev_info;

        memset(&dev_info, 0, sizeof(dev_info));
        rte_eth_dev_info_get(i, &dev_info);

        printf("DPDK port_id %d (%s)   RXQ %hu TXQ %hu  ", i, dev_info.driver_name,
               dev_info.max_rx_queues, dev_info.max_tx_queues);

        if (dev_info.pci_dev) {
            printf("%04hx:%02hhx:%02hhx.%02hhx %04hx:%04hx  ", dev_info.pci_dev->addr.domain,
                   dev_info.pci_dev->addr.bus, dev_info.pci_dev->addr.devid, dev_info.pci_dev->addr.function,
                   dev_info.pci_dev->id.vendor_id, dev_info.pci_dev->id.device_id);
        }

        printf("\n");
    }
}

static int log_eth_dev_info(struct rte_eth_dev_info* dev_info) {
//    uint8_t i;
	if (!dev_info) return -1;
	RTE_LOG(DEBUG, PMD, "driver_name: %s (if_index: %d)\n", dev_info->driver_name, dev_info->if_index);
	RTE_LOG(DEBUG, PMD, "nb_rx_queues: %d\n", dev_info->nb_rx_queues);
	RTE_LOG(DEBUG, PMD, "nb_tx_queues: %d\n", dev_info->nb_tx_queues);
	RTE_LOG(DEBUG, PMD, "rx_offload_capa: %lx\n", dev_info->rx_offload_capa);
	RTE_LOG(DEBUG, PMD, "flow_type_rss_offloads: %lx\n", dev_info->flow_type_rss_offloads);
//    for (i = 0; i < params->nb_kni; i++)
//    	RTE_LOG(DEBUG, PMD, "lcore_k[%d]: %d\n", i, dev_info->lcore_k[i]);
    return 0;
}

static int log_eth_rxconf(struct rte_eth_rxconf* rxconf) {
	if (!rxconf) return -1;
	RTE_LOG(DEBUG, PMD, "rx_thresh (p,h,w): (%d, %d, %d)\n", rxconf->rx_thresh.pthresh,
			rxconf->rx_thresh.hthresh, rxconf->rx_thresh.wthresh);
	RTE_LOG(DEBUG, PMD, "rx_free_thresh: %d\n", rxconf->rx_free_thresh);
	RTE_LOG(DEBUG, PMD, "rx_drop_en: %d\n", rxconf->rx_drop_en);
	RTE_LOG(DEBUG, PMD, "rx_deferred_start: %d\n", rxconf->rx_deferred_start);

    return 0;
}


#define CHECK_INTERVAL  100  /* 100ms */
#define MAX_REPEAT_TIMES 90  /* 9s (90 * 100ms) in total */

static void
assert_link_status(int port_id)
{
    struct rte_eth_link link;
    uint8_t rep_cnt = MAX_REPEAT_TIMES;

    memset(&link, 0, sizeof(link));
    do {
        rte_eth_link_get(port_id, &link);
        if (link.link_status == ETH_LINK_UP)
            break;
        rte_delay_ms(CHECK_INTERVAL);
    } while (--rep_cnt);

    if (link.link_status == ETH_LINK_DOWN)
        rte_exit(EXIT_FAILURE, ":: error: link is still down\n");
}


int init_pmd_port(int port, int rxqs, int txqs, int rxq_core[], int txq_core[], int nrxd, int ntxd,
                  int loopback, int tso, int csumoffload, struct rte_fdir_conf const* p_fdir_conf) {
    struct rte_eth_dev_info dev_info = {};
    struct rte_eth_conf eth_conf;
    struct rte_eth_rxconf eth_rxconf;
    struct rte_eth_txconf eth_txconf;
    int ret, i;

    /* Need to access rte_eth_devices manually since DPDK currently
     * provides no other mechanism for checking whether something is
     * attached */
    if (port >= RTE_MAX_ETHPORTS || (rte_eth_devices[port].state != RTE_ETH_DEV_ATTACHED) ) {
        printf("Port not found %d\n", port);
        return -ENODEV;
    }

    eth_conf           = default_eth_conf;
    eth_conf.lpbk_mode = !(!loopback);
    if (p_fdir_conf) eth_conf.fdir_conf = *p_fdir_conf;

    /* Use default rx/tx configuration as provided by PMD drivers,
     * with minor tweaks */
    rte_eth_dev_info_get(port, &dev_info);

    eth_conf.rx_adv_conf.rss_conf.rss_hf = dev_info.flow_type_rss_offloads;


    eth_rxconf = dev_info.default_rxconf;
    /* Drop packets when no descriptors are available */
    //eth_rxconf.rx_drop_en = 0; // changed that to 0, because 82574L seems not supporting this
    //eth_rxconf.rx_drop_en = 1;
    //eth_rxconf.rx_thresh.pthresh=RX_PTHRESH;
    //eth_rxconf.rx_thresh.hthresh=RX_HTHRESH;
    //eth_rxconf.rx_thresh.wthresh=RX_WTHRESH;
    //eth_rxconf.rx_free_thresh=RX_FREE_THRESH;

    eth_txconf           = dev_info.default_txconf;
    tso                  = !(!tso);
    csumoffload          = !(!csumoffload);
    eth_txconf.txq_flags = ETH_TXQ_FLAGS_NOVLANOFFL | ETH_TXQ_FLAGS_NOMULTSEGS * (1 - tso) |
                           ETH_TXQ_FLAGS_NOXSUMS * (1 - csumoffload);

    ret = rte_eth_dev_configure(port, rxqs, txqs, &eth_conf);
    rte_eth_dev_info_get(port, &dev_info);

    // some logging:
    RTE_LOG(DEBUG, PMD, "rte_eth_dev_info:\n");
    log_eth_dev_info(&dev_info);
    RTE_LOG(DEBUG, PMD, "default eth_rxconf:\n");
    log_eth_rxconf(&dev_info.default_rxconf);
    RTE_LOG(DEBUG, PMD, "using eth_rxconf:\n");
    log_eth_rxconf(&eth_rxconf);

    if (ret != 0) {
        RTE_LOG(CRIT, PMD, "Failed to configure port \n");
        return ret; /* Don't need to clean up here */
    }

    /* Set to promiscuous mode */
    rte_eth_promiscuous_enable(port);

    for (i = 0; i < rxqs; i++) {
        int sid = rte_lcore_to_socket_id(rxq_core[i]);
        ret = rte_eth_rx_queue_setup(port, i, nrxd, sid, &eth_rxconf, get_pframe_pool(rxq_core[i], sid));
        if (ret != 0) {
            RTE_LOG(CRIT, PMD,"Failed to initialize rxq\n");
            return ret; /* Clean things up? */
        }
    }

    for (i = 0; i < txqs; i++) {
        int sid = rte_lcore_to_socket_id(txq_core[i]);

        ret = rte_eth_tx_queue_setup(port, i, ntxd, sid, &eth_txconf);
        if (ret != 0) {
            RTE_LOG(CRIT, PMD,"Failed to initialize txq\n");
            return ret; /* Clean things up */
        }
    }
    ret = rte_eth_dev_start(port);
    if (ret != 0) {
        RTE_LOG(CRIT, PMD, "Failed to configure port \n");
        return ret; /* Clean up things */
    }

    assert_link_status(port);
    RTE_LOG(INFO, PMD, "pmd port %d configured successfully\n", port);

    return 0;
}

void free_pmd_port(int port) {
    rte_eth_dev_stop(port);
    rte_eth_dev_close(port);
}

uint32_t eth_rx_burst(int port, int qid, mbuf_array_t pkts, uint16_t len) {
	uint32_t ret = rte_eth_rx_burst((uint16_t) port, (uint16_t) qid, (struct rte_mbuf**)pkts, len);
/* Removed prefetching since the benefit in performance for single core was
 * outweighed by the loss in performance with several cores. */
#if 0
    for (int i = 0; i < ret; i++) {
        rte_prefetch0(rte_pktmbuf_mtod(pkts[i], void*));
    }
#endif
    return ret;
}

uint32_t eth_tx_burst(int port, int qid, mbuf_array_t pkts, uint16_t len) {
	return rte_eth_tx_burst((uint16_t) port, (uint16_t)qid, (struct rte_mbuf**)pkts, len);
}

int find_port_with_pci_address(const char* pci) {
    struct rte_pci_addr addr;
    struct rte_eth_dev_info info;
    char devargs[1024];
    int ret;
    uint16_t port_id;

    // Cannot parse address
    if (eal_parse_pci_DomBDF(pci, &addr) != 0 && eal_parse_pci_BDF(pci, &addr) != 0) {
        return -1;
    }

    for (int i = 0; i < RTE_MAX_ETHPORTS; i++) {
        if (rte_eth_devices[i].state!= RTE_ETH_DEV_ATTACHED) {
            continue;
        }
/* TODO
        if (!rte_eth_devices[i].pci_dev) {
            continue;
        }

        if (rte_eal_compare_pci_addr(&addr, &rte_eth_devices[i].pci_dev->addr)) {
            continue;
        }
*/
        // needs still testing:
        if (get_rte_eth_dev_info(i, &info)) {
        	continue;
        }
        if (rte_eal_compare_pci_addr(&addr, &info.pci_dev->addr)) {
            continue;
        }


        return i;
    }

    /* If not found, maybe the device has not been attached yet */

    snprintf(devargs, 1024, "%04x:%02x:%02x.%02x", addr.domain, addr.bus, addr.devid, addr.function);

    ret = rte_eth_dev_attach(devargs, &port_id);

    if (ret < 0) {
        return -1;
    }
    return (int)port_id;
}

/* Attach a device with a given name (useful when attaching virtual devices). Returns either the
   port number of the
   device or an error if not found. */
int attach_pmd_device(const char* devname) {
    uint16_t port = 0;
    int error = rte_eth_dev_attach(devname, &port);

    if (error != 0) {
        // Could not attach
        return -ENODEV;
    }
    return (int)port;
}

/* Add function to modify RSS hash function using
 * rte_eth_dev_rss_hash_update */

#include <assert.h>
#include <numa.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <rte_bus_pci.h>
#include <rte_kni.h>

#include "mempool.h"


/* Total octets in ethernet header */
#define KNI_ENET_HEADER_SIZE    14

/* Total octets in the FCS */
#define KNI_ENET_FCS_SIZE       4

/* Macros for printing using RTE_LOG */
#define RTE_LOGTYPE_APP RTE_LOGTYPE_USER1


#define MAX_ARGS 128

/* Options for configuring ethernet port */
static struct rte_eth_conf port_conf = {
	.rxmode = {
		.header_split = 0,      /* Header Split disabled */
		.hw_ip_checksum = 0,    /* IP checksum offload disabled */
		.hw_vlan_filter = 0,    /* VLAN filtering disabled */
		.jumbo_frame = 0,       /* Jumbo Frame Support disabled */
		.hw_strip_crc = 1,      /* CRC stripped by hardware */
	},
	.txmode = {
		.mq_mode = ETH_MQ_TX_NONE,
	},
};


#define KNI_MAX_KTHREAD 32
/*
 * Structure of port parameters
*/
struct kni_port_params {
        uint16_t port_id;/* Port ID */
        unsigned lcore_rx; /* lcore ID for RX */
        unsigned lcore_tx; /* lcore ID for TX */
        uint32_t nb_lcore_k; /* Number of lcores for KNI multi kernel threads */
        uint32_t nb_kni; /* Number of KNI devices to be created, used internally */
        unsigned lcore_k[KNI_MAX_KTHREAD]; /* lcore ID list for kthreads */
        struct rte_kni *kni[KNI_MAX_KTHREAD]; /* KNI context pointers */
} __rte_cache_aligned;


static int log_kni_port_params(struct kni_port_params* params) {
    uint8_t i;
	if (!params) return -1;
	RTE_LOG(DEBUG,KNI, "port_id: %d\n", params->port_id);
	RTE_LOG(DEBUG,KNI, "lcore_rx: %d\n", params->lcore_rx);
	RTE_LOG(DEBUG,KNI, "lcore_tx: %d\n", params->lcore_tx);
	RTE_LOG(DEBUG,KNI, "nb_lcore_k: %d\n", params->nb_lcore_k);
	RTE_LOG(DEBUG,KNI, "nb_kni: %d\n", params->nb_kni);
    for (i = 0; i < params->nb_kni; i++)
    	RTE_LOG(DEBUG,KNI, "lcore_k[%d]: %d\n", i, params->lcore_k[i]);
    return 0;
}

/* Callback for request of changing MTU */
static int
kni_change_mtu(uint16_t port_id, unsigned new_mtu)
{
	int ret;
	struct rte_eth_conf conf;

	if (port_id >= rte_eth_dev_count()) {
		RTE_LOG(ERR, APP, "Invalid port id %d\n", port_id);
		return -EINVAL;
	}

	RTE_LOG(INFO, APP, "Change MTU of port %d to %u\n", port_id, new_mtu);

	/* Stop specific port */
	rte_eth_dev_stop(port_id);

	memcpy(&conf, &port_conf, sizeof(conf));
	/* Set new MTU */
	if (new_mtu > ETHER_MAX_LEN)
		conf.rxmode.jumbo_frame = 1;
	else
		conf.rxmode.jumbo_frame = 0;

	/* mtu + length of header + length of FCS = max pkt length */
	conf.rxmode.max_rx_pkt_len = new_mtu + KNI_ENET_HEADER_SIZE +
							KNI_ENET_FCS_SIZE;
	ret = rte_eth_dev_configure(port_id, 1, 1, &conf);
	if (ret < 0) {
		RTE_LOG(ERR, APP, "Fail to reconfigure port %d\n", port_id);
		return ret;
	}

	/* Restart specific port */
	ret = rte_eth_dev_start(port_id);
	if (ret < 0) {
		RTE_LOG(ERR, APP, "Fail to restart port %d\n", port_id);
		return ret;
	}

	return 0;
}

/* Callback for request of configuring network interface up/down */
static int
kni_config_network_interface(uint16_t port_id, uint8_t if_up)
{
	int ret = 0;

	if (port_id >= rte_eth_dev_count() || port_id >= RTE_MAX_ETHPORTS) {
		RTE_LOG(ERR, APP, "Invalid port id %d\n", port_id);
		return -EINVAL;
	}

	RTE_LOG(INFO, APP, "Configure network interface of %d %s\n",
					port_id, if_up ? "up" : "down");

	if (if_up != 0) { /* Configure network interface up */
		rte_eth_dev_stop(port_id);
		ret = rte_eth_dev_start(port_id);
	} else /* Configure network interface down */
		rte_eth_dev_stop(port_id);

	if (ret < 0)
		RTE_LOG(ERR, APP, "Failed to start port %d\n", port_id);

	return ret;
}



 struct rte_kni* kni_alloc(uint16_t port_id, struct kni_port_params* params)
 {
     uint8_t i;
     struct rte_kni *kni=(struct rte_kni*) -1;
     struct rte_kni_conf conf;

     if (port_id >= RTE_MAX_ETHPORTS || !params)
         return (struct rte_kni*) -1;

     params->nb_kni = params->nb_lcore_k ? params->nb_lcore_k : 1;

     log_kni_port_params(params);

     for (i = 0; i < params->nb_kni; i++) {

         /* Clear conf at first */

         memset(&conf, 0, sizeof(conf));
         if (params->nb_lcore_k) {
             snprintf(conf.name, RTE_KNI_NAMESIZE, "vEth%u_%u", port_id, i);
             conf.core_id = params->lcore_k[i];
             conf.force_bind = 1;
         }
         else snprintf(conf.name, RTE_KNI_NAMESIZE, "vEth%u", port_id);
		 conf.group_id = (uint16_t)port_id;
		 conf.mbuf_size = RTE_MBUF_DEFAULT_BUF_SIZE;

		 /*
		  *   The first KNI device associated to a port
		  *   is the master, for multiple kernel thread
		  *   environment.
		  */

		 if (i == 0) {
			 struct rte_kni_ops ops;
			 struct rte_eth_dev_info dev_info;


			 memset(&dev_info, 0, sizeof(dev_info));
			 rte_eth_dev_info_get(port_id, &dev_info);
			 if (dev_info.pci_dev) {
				conf.addr = dev_info.pci_dev->addr;
				conf.id = dev_info.pci_dev->id;
			 }

			 memset(&ops, 0, sizeof(ops));

			 ops.port_id = port_id;
			 ops.change_mtu = kni_change_mtu;
			 ops.config_network_if = kni_config_network_interface;

			 kni = rte_kni_alloc(get_mempool_for_core(conf.core_id), &conf, &ops);
		 } else
			 kni = rte_kni_alloc(get_mempool_for_core(conf.core_id), &conf, NULL);

		 if (!kni)
			 rte_exit(EXIT_FAILURE, "Fail to create kni for "
					 "port: %d\n", port_id);

		 params->kni[i] = kni;
     }
     return kni;
}



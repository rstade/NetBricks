/* SPDX-License-Identifier: BSD-3-Clause
 * Copyright(c) 2010-2016 Intel Corporation.
 * Copyright 2013-2014 6WIND S.A.
 */

 /* from dpdk/app/test-pmd/config.c (DPDK 18.02.2) , modified by Rainer Stademann */

#include <stdint.h>
#include <rte_ethdev.h>
#include <rte_eth_ctrl.h>
#include <rte_pmd_ixgbe.h>
#include <rte_pmd_i40e.h>
#include "fdir.h"


#define RTE_PORT_ALL            (~(portid_t)0x0)

static char *
flowtype_to_str(uint16_t flow_type)
{
	struct flow_type_info {
		char str[32];
		uint16_t ftype;
	};

	uint8_t i;
	static struct flow_type_info flowtype_str_table[] = {
		{"raw", RTE_ETH_FLOW_RAW},
		{"ipv4", RTE_ETH_FLOW_IPV4},
		{"ipv4-frag", RTE_ETH_FLOW_FRAG_IPV4},
		{"ipv4-tcp", RTE_ETH_FLOW_NONFRAG_IPV4_TCP},
		{"ipv4-udp", RTE_ETH_FLOW_NONFRAG_IPV4_UDP},
		{"ipv4-sctp", RTE_ETH_FLOW_NONFRAG_IPV4_SCTP},
		{"ipv4-other", RTE_ETH_FLOW_NONFRAG_IPV4_OTHER},
		{"ipv6", RTE_ETH_FLOW_IPV6},
		{"ipv6-frag", RTE_ETH_FLOW_FRAG_IPV6},
		{"ipv6-tcp", RTE_ETH_FLOW_NONFRAG_IPV6_TCP},
		{"ipv6-udp", RTE_ETH_FLOW_NONFRAG_IPV6_UDP},
		{"ipv6-sctp", RTE_ETH_FLOW_NONFRAG_IPV6_SCTP},
		{"ipv6-other", RTE_ETH_FLOW_NONFRAG_IPV6_OTHER},
		{"l2_payload", RTE_ETH_FLOW_L2_PAYLOAD},
		{"port", RTE_ETH_FLOW_PORT},
		{"vxlan", RTE_ETH_FLOW_VXLAN},
		{"geneve", RTE_ETH_FLOW_GENEVE},
		{"nvgre", RTE_ETH_FLOW_NVGRE},
	};

	for (i = 0; i < RTE_DIM(flowtype_str_table); i++) {
		if (flowtype_str_table[i].ftype == flow_type)
			return flowtype_str_table[i].str;
	}

	return NULL;
}

static inline void
print_fdir_mask(struct rte_eth_fdir_info *info)
{
    struct rte_eth_fdir_masks *mask=&info->mask;
	printf("\n    vlan_tci: 0x%04x", rte_be_to_cpu_16(mask->vlan_tci_mask));

	if (info->mode == RTE_FDIR_MODE_PERFECT_TUNNEL)
		printf(", mac_addr: 0x%02x, tunnel_type: 0x%01x,"
			" tunnel_id: 0x%08x",
			mask->mac_addr_byte_mask, mask->tunnel_type_mask,
			rte_be_to_cpu_32(mask->tunnel_id_mask));
	else if (info->mode != RTE_FDIR_MODE_PERFECT_MAC_VLAN) {
		printf(", src_ipv4: 0x%08x, dst_ipv4: 0x%08x",
			rte_be_to_cpu_32(mask->ipv4_mask.src_ip),
			rte_be_to_cpu_32(mask->ipv4_mask.dst_ip));

		printf("\n    src_port: 0x%04x, dst_port: 0x%04x",
			rte_be_to_cpu_16(mask->src_port_mask),
			rte_be_to_cpu_16(mask->dst_port_mask));

		printf("\n    src_ipv6: 0x%08x,0x%08x,0x%08x,0x%08x",
			rte_be_to_cpu_32(mask->ipv6_mask.src_ip[0]),
			rte_be_to_cpu_32(mask->ipv6_mask.src_ip[1]),
			rte_be_to_cpu_32(mask->ipv6_mask.src_ip[2]),
			rte_be_to_cpu_32(mask->ipv6_mask.src_ip[3]));

		printf("\n    dst_ipv6: 0x%08x,0x%08x,0x%08x,0x%08x",
			rte_be_to_cpu_32(mask->ipv6_mask.dst_ip[0]),
			rte_be_to_cpu_32(mask->ipv6_mask.dst_ip[1]),
			rte_be_to_cpu_32(mask->ipv6_mask.dst_ip[2]),
			rte_be_to_cpu_32(mask->ipv6_mask.dst_ip[3]));
	}

	printf("\n");
}

static inline void
print_fdir_flex_payload(struct rte_eth_fdir_flex_conf *flex_conf, uint32_t num)
{
	struct rte_eth_flex_payload_cfg *cfg;
	uint32_t i, j;

	for (i = 0; i < flex_conf->nb_payloads; i++) {
		cfg = &flex_conf->flex_set[i];
		if (cfg->type == RTE_ETH_RAW_PAYLOAD)
			printf("\n    RAW:  ");
		else if (cfg->type == RTE_ETH_L2_PAYLOAD)
			printf("\n    L2_PAYLOAD:  ");
		else if (cfg->type == RTE_ETH_L3_PAYLOAD)
			printf("\n    L3_PAYLOAD:  ");
		else if (cfg->type == RTE_ETH_L4_PAYLOAD)
			printf("\n    L4_PAYLOAD:  ");
		else
			printf("\n    UNKNOWN PAYLOAD(%u):  ", cfg->type);
		for (j = 0; j < num; j++)
			printf("  %-5u", cfg->src_offset[j]);
	}
	printf("\n");
}

static inline void
print_fdir_flex_mask(struct rte_eth_fdir_flex_conf *flex_conf, uint32_t num)
{
	struct rte_eth_fdir_flex_mask *mask;
	uint32_t i, j;
	char *p;

	for (i = 0; i < flex_conf->nb_flexmasks; i++) {
		mask = &flex_conf->flex_mask[i];
		p = flowtype_to_str(mask->flow_type);
		printf("\n    %s:\t", p ? p : "unknown");
		for (j = 0; j < num; j++)
			printf(" %02x", mask->mask[j]);
	}
	printf("\n");
}

static inline void
print_fdir_flow_type(uint32_t flow_types_mask)
{
	int i;
	char *p;

	for (i = RTE_ETH_FLOW_UNKNOWN; i < RTE_ETH_FLOW_MAX; i++) {
		if (!(flow_types_mask & (1 << i)))
			continue;
		p = flowtype_to_str(i);
		if (p)
			printf(" %s", p);
		else
			printf(" unknown");
	}
	printf("\n");
}

enum print_warning {
    ENABLED_WARN = 0,
    DISABLED_WARN
};

int
port_id_is_invalid(portid_t port_id, enum print_warning warning)
{
    uint16_t pid;

    if (port_id == (portid_t)RTE_PORT_ALL)
        return 0;

    RTE_ETH_FOREACH_DEV(pid)
        if (port_id == pid)
            return 0;

    if (warning == ENABLED_WARN)
        printf("Invalid port %d\n", port_id);

    return 1;
}

static int
get_fdir_info(portid_t port_id, struct rte_eth_fdir_info *fdir_info,
              struct rte_eth_fdir_stats *fdir_stat)
{
    int ret = -ENOTSUP;

#ifdef RTE_NET_I40E
    if (ret == -ENOTSUP) {
        ret = rte_pmd_i40e_get_fdir_info(port_id, fdir_info);
        if (!ret)
            ret = rte_pmd_i40e_get_fdir_stats(port_id, fdir_stat);
    }
#endif
#ifdef RTE_NET_IXGBE
    if (ret == -ENOTSUP) {
        ret = rte_pmd_ixgbe_get_fdir_info(port_id, fdir_info);
        if (!ret)
            ret = rte_pmd_ixgbe_get_fdir_stats(port_id, fdir_stat);
    }
#endif
    switch (ret) {
        case 0:
            break;
        case -ENOTSUP:
            printf("\n FDIR is not supported on port %-2d\n",
                   port_id);
            break;
        default:
            printf("programming error: (%s)\n", strerror(-ret));
            break;
    }
    return ret;
}


void
fdir_get_infos(portid_t port_id)
{
    struct rte_eth_fdir_stats fdir_stat;
    struct rte_eth_fdir_info fdir_info;

    static const char *fdir_stats_border = "########################";

    if (port_id_is_invalid(port_id, ENABLED_WARN))
        return;

    memset(&fdir_info, 0, sizeof(fdir_info));
    memset(&fdir_stat, 0, sizeof(fdir_stat));
    if (get_fdir_info(port_id, &fdir_info, &fdir_stat))
        return;

    printf("\n  %s FDIR infos for port %-2d     %s\n",
           fdir_stats_border, port_id, fdir_stats_border);
    printf("  MODE: ");
    if (fdir_info.mode == RTE_FDIR_MODE_PERFECT)
        printf("  PERFECT\n");
    else if (fdir_info.mode == RTE_FDIR_MODE_PERFECT_MAC_VLAN)
        printf("  PERFECT-MAC-VLAN\n");
    else if (fdir_info.mode == RTE_FDIR_MODE_PERFECT_TUNNEL)
        printf("  PERFECT-TUNNEL\n");
    else if (fdir_info.mode == RTE_FDIR_MODE_SIGNATURE)
        printf("  SIGNATURE\n");
    else
        printf("  DISABLE\n");
    if (fdir_info.mode != RTE_FDIR_MODE_PERFECT_MAC_VLAN
        && fdir_info.mode != RTE_FDIR_MODE_PERFECT_TUNNEL) {
        printf("  SUPPORTED FLOW TYPE: ");
        print_fdir_flow_type(fdir_info.flow_types_mask[0]);
    }
    printf("  FLEX PAYLOAD INFO:\n");
    printf("  max_len:       %-10"PRIu32"  payload_limit: %-10"PRIu32"\n"
           "  payload_unit:  %-10"PRIu32"  payload_seg:   %-10"PRIu32"\n"
           "  bitmask_unit:  %-10"PRIu32"  bitmask_num:   %-10"PRIu32"\n",
           fdir_info.max_flexpayload, fdir_info.flex_payload_limit,
           fdir_info.flex_payload_unit,
           fdir_info.max_flex_payload_segment_num,
           fdir_info.flex_bitmask_unit, fdir_info.max_flex_bitmask_num);
    printf("  MASK: ");
    print_fdir_mask(&fdir_info);
    if (fdir_info.flex_conf.nb_payloads > 0) {
        printf("  FLEX PAYLOAD SRC OFFSET:");
        print_fdir_flex_payload(&fdir_info.flex_conf, fdir_info.max_flexpayload);
    }
    if (fdir_info.flex_conf.nb_flexmasks > 0) {
        printf("  FLEX MASK CFG:");
        print_fdir_flex_mask(&fdir_info.flex_conf, fdir_info.max_flexpayload);
    }
    printf("  guarant_count: %-10"PRIu32"  best_count:    %"PRIu32"\n",
           fdir_stat.guarant_cnt, fdir_stat.best_cnt);
    printf("  guarant_space: %-10"PRIu32"  best_space:    %"PRIu32"\n",
           fdir_info.guarant_spc, fdir_info.best_spc);
    printf("  collision:     %-10"PRIu32"  free:          %"PRIu32"\n"
           "  maxhash:       %-10"PRIu32"  maxlen:        %"PRIu32"\n"
           "  add:	         %-10"PRIu64"  remove:        %"PRIu64"\n"
           "  f_add:         %-10"PRIu64"  f_remove:      %"PRIu64"\n",
           fdir_stat.collision, fdir_stat.free,
           fdir_stat.maxhash, fdir_stat.maxlen,
           fdir_stat.add, fdir_stat.remove,
           fdir_stat.f_add, fdir_stat.f_remove);
    printf("  %s############################%s\n",
           fdir_stats_border, fdir_stats_border);
}


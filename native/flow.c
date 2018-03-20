//
// Modified by rainer on 17.03.18.
//

/*-
 *   BSD LICENSE
 *
 *   Copyright 2017 Mellanox.
 *
 *   Redistribution and use in source and binary forms, with or without
 *   modification, are permitted provided that the following conditions
 *   are met:
 *
 *     * Redistributions of source code must retain the above copyright
 *       notice, this list of conditions and the following disclaimer.
 *     * Redistributions in binary form must reproduce the above copyright
 *       notice, this list of conditions and the following disclaimer in
 *       the documentation and/or other materials provided with the
 *       distribution.
 *     * Neither the name of Mellanox nor the names of its
 *       contributors may be used to endorse or promote products derived
 *       from this software without specific prior written permission.
 *
 *   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 *   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 *   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
 *   A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 *   OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 *   SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
 *   LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 *   DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 *   THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 *   (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 *   OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
#include <stdint.h>

#include <rte_flow.h>

#define MAX_PATTERN_NUM		4

struct rte_flow *
generate_tcp_flow       (uint16_t port_id, uint16_t rx_q,
                         uint32_t src_ip, uint32_t src_mask,
                         uint32_t dest_ip, uint32_t dest_mask,
                         uint16_t src_port, uint16_t src_port_mask,
                         uint16_t dst_port, uint16_t dst_port_mask,
                         struct rte_flow_error *error);


/**
 * create a flow rule that sends packets with matching src and dest ip
 * to selected queue.
 *
 * @param port_id
 *   The selected port.
 * @param rx_q
 *   The selected target queue.
 * @param src_ip
 *   The src ip value to match the input packet.
 * @param src_mask
 *   The mask to apply to the src ip.
 * @param dst_ip
 *   The dest ip value to match the input packet.
 * @param dst_mask
 *   The mask to apply to the dest ip.
 * @param[out] error
 *   Perform verbose error reporting if not NULL.
 *
 * @return
 *   A flow if the rule could be created else return NULL.
 */
struct rte_flow *
generate_tcp_flow (uint16_t port_id, uint16_t rx_q,
                   uint32_t src_ip, uint32_t src_mask,
                   uint32_t dst_ip, uint32_t dst_mask,
                   uint16_t src_port, uint16_t src_port_mask,
                   uint16_t dst_port, uint16_t dst_port_mask,
                   struct rte_flow_error *error)
{
    struct rte_flow_attr attr;
    struct rte_flow_item pattern[MAX_PATTERN_NUM];
    struct rte_flow_action action[MAX_PATTERN_NUM];
    struct rte_flow *flow = NULL;
    struct rte_flow_action_queue queue = { .index = rx_q };
//    struct rte_flow_item_eth eth_spec;
//    struct rte_flow_item_eth eth_mask;
//    struct rte_flow_item_vlan vlan_spec;
//    struct rte_flow_item_vlan vlan_mask;
    struct rte_flow_item_ipv4 ip_spec;
    struct rte_flow_item_ipv4 ip_mask;
    struct rte_flow_item_tcp tcp_spec;
    struct rte_flow_item_tcp tcp_mask;
    int res;

    memset(pattern, 0, sizeof(pattern));
    memset(action, 0, sizeof(action));

    /*
     * set the rule attribute.
     * in this case only ingress packets will be checked.
     */
    memset(&attr, 0, sizeof(struct rte_flow_attr));
    attr.ingress = 1;

    /*
     * create the action sequence.
     * one action only,  move packet to queue
     */

    action[0].type = RTE_FLOW_ACTION_TYPE_QUEUE;
    action[0].conf = &queue;
    action[1].type = RTE_FLOW_ACTION_TYPE_END;

    /*
     * set the first level of the pattern (eth).
     * since in this example we just want to get the
     * ipv4 we set this level to allow all.
     */
//    memset(&eth_spec, 0, sizeof(struct rte_flow_item_eth));
//    memset(&eth_mask, 0, sizeof(struct rte_flow_item_eth));
//    eth_spec.type = 0;
//    eth_mask.type = 0;
//    pattern[0].type = RTE_FLOW_ITEM_TYPE_ETH;
//    pattern[0].spec = &eth_spec;
//    pattern[0].mask = &eth_mask;

    /*
     * setting the second level of the pattern (vlan).
     * since in this example we just want to get the
     * ipv4 we also set this level to allow all.
     */
 //   memset(&vlan_spec, 0, sizeof(struct rte_flow_item_vlan));
 //   memset(&vlan_mask, 0, sizeof(struct rte_flow_item_vlan));
 //   pattern[1].type = RTE_FLOW_ITEM_TYPE_VLAN;
 //   pattern[1].spec = &vlan_spec;
 //   pattern[1].mask = &vlan_mask;

    /*
     * setting the third level of the pattern (ip).
     * in this example this is the level we care about
     * so we set it according to the parameters.
     */
//    int debug_level= rte_log_get_level(RTE_LOGTYPE_PMD);
//    printf("**************  debug level: %d\n", debug_level);
//    RTE_LOG(ERR, PMD, "*****************   test log message\n");

    memset(&ip_spec, 0, sizeof(struct rte_flow_item_ipv4));
    memset(&ip_mask, 0, sizeof(struct rte_flow_item_ipv4));
    ip_spec.hdr.dst_addr = htonl(dst_ip);
    ip_mask.hdr.dst_addr = htonl(dst_mask);
    ip_spec.hdr.src_addr = htonl(src_ip);
    ip_mask.hdr.src_addr = htonl(src_mask);
    ip_spec.hdr.next_proto_id = 0x06;
    //ip_mask.hdr.next_proto_id = 0xff;  no mask for proto possible for fdir filter on x520
    pattern[0].type = RTE_FLOW_ITEM_TYPE_IPV4;
    pattern[0].spec = &ip_spec;
    pattern[0].mask = &ip_mask;

    RTE_LOG(DEBUG, PMD, "dst ip %08x, mask: %08x\n", ip_spec.hdr.dst_addr, ip_mask.hdr.dst_addr);
    RTE_LOG(DEBUG, PMD, "src ip %08x, mask: %08x\n", ip_spec.hdr.src_addr, ip_mask.hdr.src_addr);

    memset(&tcp_spec, 0, sizeof(tcp_spec));
    tcp_spec.hdr.src_port = htons(src_port);
    tcp_spec.hdr.dst_port = htons(dst_port);
    memset(&tcp_mask, 0, sizeof(tcp_mask));
    tcp_mask.hdr.src_port = htons(src_port_mask);
    tcp_mask.hdr.dst_port = htons(dst_port_mask);
//    tcp_mask.hdr.src_port = 0x0000;
//    tcp_mask.hdr.dst_port = 0xffff;

    RTE_LOG(DEBUG, PMD, "dst port %04x, mask: %04x\n", tcp_spec.hdr.dst_port, tcp_mask.hdr.dst_port);
    RTE_LOG(DEBUG, PMD, "src port %04x, mask: %04x\n", tcp_spec.hdr.src_port, tcp_mask.hdr.src_port);
    pattern[1].type = RTE_FLOW_ITEM_TYPE_TCP;
    pattern[1].spec = &tcp_spec;
    pattern[1].mask = &tcp_mask;
    pattern[1].last = NULL;


    /* the final level must be always type end */
    pattern[2].type = RTE_FLOW_ITEM_TYPE_END;

    res = rte_flow_validate(port_id, &attr, pattern, action, error);
    if (!res)
        RTE_LOG(DEBUG, PMD, "flow validation succeeded\n");
        flow = rte_flow_create(port_id, &attr, pattern, action, error);

    return flow;
}


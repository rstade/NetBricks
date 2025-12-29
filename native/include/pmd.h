#ifndef __PMD_H__
#define __PMD_H__
int num_pmd_ports();
int get_pmd_ports(struct rte_eth_dev_info* info, int len);
void enumerate_pmd_ports();
int init_pmd_port(uint16_t port, uint16_t rxqs, uint16_t txqs, int rxq_core[], int txq_core[], uint16_t nrxd, uint16_t ntxd,
                  int loopback, int tso, int csumoffload, enum rte_eth_rx_mq_mode rx_mq_mode, uint8_t rss_key[], uint16_t key_len, struct rte_fdir_conf const *p_fdir_conf);
void free_pmd_port(uint16_t port);
int recv_pkts(int port, int qid, mbuf_array_t pkts, int len);
int send_pkts(int port, int qid, mbuf_array_t pkts, int len);
#endif

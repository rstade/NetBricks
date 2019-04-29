use super::{DriverType, NetbricksConfiguration, PortConfiguration};
use super::super::interface::{FlowSteeringMode, NetSpec};
use common::errors;
use common::errors::ErrorKind;
use native::zcsi::{RteEthIpv4Flow, RteFdirConf, RteFdirMode, RteFdirPballocType};
use std::clone::Clone;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::net::{AddrParseError, Ipv4Addr};
use std::option::Option::Some;
use std::result::Result::Err;
use std::str::FromStr;
use std::string::String;
use std::string::ToString;
use ipnet::Ipv4Net;
use eui48::MacAddress;
use toml::{self, Value};

/// Default configuration values
pub const DEFAULT_MBUF_CNT: u32 = 65535;
pub const DEFAULT_POOL_SIZE: u32 = 2048;
pub const DEFAULT_CACHE_SIZE: u32 = 32;
pub const DEFAULT_SECONDARY: bool = false;
pub const DEFAULT_PRIMARY_CORE: i32 = 0;
pub const DEFAULT_NAME: &'static str = "zcsi";
pub const NUM_RXD: u16 = 128;
pub const NUM_TXD: u16 = 128;

/// Read a TOML stub and figure out the port.
fn read_port(value: &Value) -> errors::Result<PortConfiguration> {
    match *value {
        Value::Table(ref port_def) => {
            let name = match port_def.get("name") {
                Some(&Value::String(ref name)) => name.clone(),
                _ => return Err(ErrorKind::ConfigurationError(String::from("Could not parse name for port")).into()),
            };

            let kni = match port_def.get("kni") {
                Some(&Value::String(ref kni)) => Some(kni.clone()),
                None => None,
                v => {
                    return Err(
                        ErrorKind::ConfigurationError(format!("Could not parse kni spec {} ", v.unwrap())).into(),
                    )
                }
            };

            let rxd = match port_def.get("rxd") {
                Some(&Value::Integer(rxd)) => rxd as u16,
                None => NUM_RXD,
                v => {
                    return Err(ErrorKind::ConfigurationError(format!(
                        "Could not parse number of rx descriptors {:?}",
                        v
                    ))
                    .into());
                }
            };

            let txd = match port_def.get("txd") {
                Some(&Value::Integer(txd)) => txd as u16,
                None => NUM_TXD,
                v => {
                    return Err(ErrorKind::ConfigurationError(format!(
                        "Could not parse number of tx descriptors {:?}",
                        v
                    ))
                    .into());
                }
            };

            let loopback = match port_def.get("loopback") {
                Some(&Value::Boolean(l)) => l,
                None => false,
                v => {
                    return Err(ErrorKind::ConfigurationError(format!("Could not parse loopback spec {:?}", v)).into())
                }
            };

            let tso = match port_def.get("tso") {
                Some(&Value::Boolean(l)) => l,
                None => false,
                v => return Err(ErrorKind::ConfigurationError(format!("Could not parse tso spec {:?}", v)).into()),
            };

            let csum = match port_def.get("checksum") {
                Some(&Value::Boolean(l)) => l,
                None => false,
                v => return Err(ErrorKind::ConfigurationError(format!("Could not parse csum spec {:?}", v)).into()),
            };

            let flow_steering: Option<FlowSteeringMode> = match port_def.get("flow_steering") {
                Some(&Value::String(ref mode)) => match &mode[..] {
                    "Ip" => Some(FlowSteeringMode::Ip),
                    "Port" => Some(FlowSteeringMode::Port),
                    _ => None,
                }
                None => None,
                _ => {
                    error!("Could not parse flow steering mode");
                    return Err(ErrorKind::ConfigurationError(String::from("Could not parse flow steering mode")).into());
                }
            };

            let ip_net = match port_def.get("ipnet") {
                Some(&Value::String(ref s_ipnet)) => s_ipnet.parse::<Ipv4Net>().ok(),
                None => None,
                v => {
                    return Err(ErrorKind::ConfigurationError(format!("Could not parse ipnet spec {:?}", v)).into())
                }
            };

            let mac = match port_def.get("mac") {
                Some(&Value::String(ref s_mac)) => s_mac.parse::<MacAddress>().ok(),
                None => None,
                v => {
                    return Err(ErrorKind::ConfigurationError(format!("Could not parse mac address {:?}", v)).into())
                }
            };

            let nsname = match port_def.get("namespace") {
                Some(&Value::String(ref s_nsname)) => Some(s_nsname.clone()),
                None => None,
                v => {
                    return Err(
                        ErrorKind::ConfigurationError(format!("Could not parse namespace {} ", v.unwrap())).into(),
                    )
                }
            };




            let symmetric_queue = port_def.contains_key("cores");
            if symmetric_queue && (port_def.contains_key("rx_cores") || port_def.contains_key("tx_cores")) {
                error!("cores specified along with rx_cores and/or tx_cores for port {}", name);
                return Err(ErrorKind::ConfigurationError(format!(
                    "cores specified along with rx_cores and/or tx_cores \
                     for port {}",
                    name
                ))
                .into());
            }

            fn read_queue(queue: &Value) -> errors::Result<Vec<i32>> {
                match *queue {
                    Value::Array(ref queues) => {
                        let mut qs = Vec::with_capacity(queues.len());
                        for q in queues {
                            if let Value::Integer(core) = *q {
                                qs.push(core as i32)
                            } else {
                                return Err(ErrorKind::ConfigurationError(format!(
                                    "Could not parse queue spec {:?}",
                                    q
                                ))
                                .into());
                            };
                        }
                        Ok(qs)
                    }
                    Value::Integer(core) => Ok(vec![core as i32]),
                    _ => Ok(vec![]),
                }
            }

            fn read_ipv4(mask_def: &BTreeMap<String, Value>, key: String) -> Result<u32, AddrParseError> {
                match mask_def.get(&key) {
                    Some(&Value::String(ref ipv4_string)) => Ok(u32::from(Ipv4Addr::from_str(ipv4_string)?)),
                    _ => Ok(0u32),
                }
            }

            fn read_hex_u32(mask_def: &BTreeMap<String, Value>, key: String) -> u32 {
                match mask_def.get(&key) {
                    Some(&Value::String(ref hex_string)) => u32::from_str_radix(hex_string, 16).unwrap_or(0u32),
                    _ => 0u32,
                }
            }

            fn read_hex_u16(mask_def: &BTreeMap<String, Value>, key: String) -> u16 {
                match mask_def.get(&key) {
                    Some(&Value::String(ref hex_string)) => u16::from_str_radix(hex_string, 16).unwrap_or(0u16),
                    _ => 0u16,
                }
            }

            fn read_hex_u8(mask_def: &BTreeMap<String, Value>, key: String) -> u8 {
                match mask_def.get(&key) {
                    Some(&Value::String(ref hex_string)) => u8::from_str_radix(hex_string, 16).unwrap_or(0u8),
                    _ => 0u8,
                }
            }

            fn read_ipv4_mask(mask_val: &Value) -> errors::Result<RteEthIpv4Flow> {
                let mut ipv4_mask = RteEthIpv4Flow {
                    src_ip: 0,
                    dst_ip: 0,
                    tos: 0,
                    ttl: 0,
                    proto: 0,
                };
                match *mask_val {
                    Value::Table(ref mask_def) => {
                        ipv4_mask.src_ip = u32::to_be(
                            read_ipv4(mask_def, "src_ip".to_string())
                                .unwrap_or(read_hex_u32(mask_def, "src_ip".to_string())),
                        );
                        ipv4_mask.dst_ip = u32::to_be(
                            read_ipv4(mask_def, "dst_ip".to_string())
                                .unwrap_or(read_hex_u32(mask_def, "dst_ip".to_string())),
                        );
                        ipv4_mask.tos = u8::to_be(read_hex_u8(mask_def, "tos".to_string()));
                        ipv4_mask.ttl = u8::to_be(read_hex_u8(mask_def, "ttl".to_string()));
                        ipv4_mask.proto = u8::to_be(read_hex_u8(mask_def, "proto".to_string()));
                        Ok(ipv4_mask)
                    }
                    _ => Err(
                        ErrorKind::ConfigurationError(String::from("Could not understand fdir ipv4_mask spec")).into(),
                    ),
                }
            }

            fn read_fdir(fdir_val: &Value) -> errors::Result<RteFdirConf> {
                let mut fdir_conf = RteFdirConf::new();
                match *fdir_val {
                    Value::Table(ref fdir_def) => {
                        match fdir_def.get("pballoc") {
                            //TODO replace unwrap() with error conversion
                            Some(v) => fdir_conf.pballoc = v.clone().try_into::<RteFdirPballocType>().unwrap(),
                            None => (), // X710 does not support pballoc
                        };
                        match fdir_def.get("mode") {
                            //TODO replace unwrap() with error conversion
                            Some(v) => fdir_conf.mode = v.clone().try_into::<RteFdirMode>().unwrap(),
                            None => {
                                return Err(ErrorKind::ConfigurationError("missing fdir mode spec".to_string()).into());
                            }
                        };
                        match fdir_def.get("ipv4_mask") {
                            Some(v) => fdir_conf.mask.ipv4_mask = read_ipv4_mask(v)?,
                            None => (),
                        };
                        fdir_conf.mask.src_port_mask = u16::to_be(read_hex_u16(fdir_def, "src_port_mask".to_string()));
                        fdir_conf.mask.dst_port_mask = u16::to_be(read_hex_u16(fdir_def, "dst_port_mask".to_string()));
                        debug!("fdir_conf: { }", fdir_conf);
                        Ok(fdir_conf)
                    }
                    _ => Err(ErrorKind::ConfigurationError(String::from("Cannot understand fdir spec")).into()),
                }
            }

            let k_cores = match port_def.get("k_cores") {
                Some(v) => try!(read_queue(v)),
                None => Vec::with_capacity(0),
            };

            let rx_queues = if symmetric_queue {
                try!(read_queue(port_def.get("cores").unwrap()))
            } else {
                match port_def.get("rx_cores") {
                    Some(v) => try!(read_queue(v)),
                    None => Vec::with_capacity(0),
                }
            };

            let tx_queues = if symmetric_queue {
                rx_queues.clone()
            } else {
                match port_def.get("tx_cores") {
                    Some(v) => read_queue(v)?,
                    None => Vec::with_capacity(0),
                }
            };

            let fdir_conf = match port_def.get("fdir") {
                Some(v) => Some(read_fdir(v)?),
                None => None,
            };

            let driver = match port_def.get("driver") {
                //TODO replace unwrap() with error conversion
                Some(v) => v.clone().try_into::<DriverType>().unwrap(),
                None => DriverType::Unknown,
            };

            let net_spec=NetSpec{
                ip_net,
                mac,
                nsname,
                ..Default::default()

            };

            let has_netspec= net_spec.mac.is_some() || net_spec.ip_net.is_some() || net_spec.port.is_some() || net_spec.nsname.is_some();

            Ok(PortConfiguration {
                name,
                rx_queues,
                tx_queues,
                rxd,
                txd,
                loopback,
                csum,
                tso,
                k_cores,
                kni,
                fdir_conf,
                flow_steering,
                driver,
                net_spec: if has_netspec { Some(net_spec) } else { None },
            })
        }
        _ => Err(ErrorKind::ConfigurationError(String::from("Could not understand port spec")).into()),
    }
}

pub fn read_toml_table(toml_value: &Value, table_name: &str) -> errors::Result<Value> {
    match toml_value.get(table_name) {
        Some(value) => Ok(value.clone()),
        _ => {
            error!("[{}] table missing", table_name);
            return Err(ErrorKind::ConfigurationError(format!("[{}] table missing", table_name)).into());
        }
    }
}

/// Read a TOML string and create a `NetbricksConfiguration` structure.
/// `configuration` is a TOML formatted string.
/// `filename` is used for error reporting purposes, and is otherwise meaningless.
pub fn read_configuration_from_str(configuration: &str, filename: &str) -> errors::Result<NetbricksConfiguration> {
    // Parse string for TOML file.
    let toml = match toml::de::from_str::<Value>(configuration) {
        Ok(toml) => toml,
        Err(error) => {
            error!("Parse error: {} in file: {}", error, filename);
            return Err(ErrorKind::ConfigurationError(format!("Experienced {} parse errors in spec.", error)).into());
        }
    };

    let toml = match read_toml_table(&toml, "netbricks") {
        Ok(value) => value,
        Err(err) => return Err(err),
    };

    // Get name from configuration
    let name = match toml.get("name") {
        Some(&Value::String(ref name)) => name.clone(),
        None => String::from(DEFAULT_NAME),
        _ => {
            error!("Could not parse name");
            return Err(ErrorKind::ConfigurationError(String::from("Could not parse name")).into());
        }
    };

    // Get primary core from configuration.
    let master_lcore = match toml.get("master_core") {
        Some(&Value::Integer(core)) => core as i32,
        Some(&Value::String(ref core)) => match core.parse() {
            Ok(c) => c,
            _ => return Err(ErrorKind::ConfigurationError(format!("Could not parse {} as core", core)).into()),
        },
        None => DEFAULT_PRIMARY_CORE,
        v => {
            error!("Could not parse core");
            return Err(ErrorKind::ConfigurationError(format!("Could not parse {:?} as core", v)).into());
        }
    };

    // Parse mempool size
    let pool_size = match toml.get("pool_size") {
        Some(&Value::Integer(pool)) => pool as u32,
        None => DEFAULT_POOL_SIZE,
        _ => {
            error!("Could not parse pool size");
            return Err(ErrorKind::ConfigurationError(String::from("Could not parse pool size")).into());
        }
    };

    // Get cache size
    let cache_size = match toml.get("cache_size") {
        Some(&Value::Integer(cache)) => cache as u32,
        None => DEFAULT_CACHE_SIZE,
        _ => {
            error!("Could not parse cache size");
            return Err(ErrorKind::ConfigurationError(String::from("Could not parse cache size")).into());
        }
    };

    // Get mbuf count for the mbuf pool
    let mbuf_cnt = match toml.get("mbuf_cnt") {
        Some(&Value::Integer(cnt)) => cnt as u32,
        None => DEFAULT_MBUF_CNT,
        _ => {
            error!("Could not parse mbuf count");
            return Err(ErrorKind::ConfigurationError(String::from("Could not parse mbuf count")).into());
        }
    };

    // Is process a secondary process
    let secondary = match toml.get("secondary") {
        Some(&Value::Boolean(secondary)) => secondary,
        None => DEFAULT_SECONDARY,
        _ => {
            error!("Could not parse whether this is a secondary process");
            return Err(ErrorKind::ConfigurationError(String::from("Could not parse secondary processor spec")).into());
        }
    };

    // Secondary ports to instantiate.
    let cores = match toml.get("cores") {
        Some(&Value::Array(ref c)) => {
            let mut cores = Vec::with_capacity(c.len());
            for core in c {
                if let Value::Integer(core) = *core {
                    cores.push(core as i32)
                } else {
                    return Err(ErrorKind::ConfigurationError(format!("Could not parse core spec {}", core)).into());
                }
            }
            cores
        }
        None => Vec::with_capacity(0),
        _ => {
            error!("Cores is not an array");
            return Err(ErrorKind::ConfigurationError(String::from("Cores is not an array")).into());
        }
    };

    let strict = match toml.get("strict") {
        Some(&Value::Boolean(l)) => l,
        None => false,
        v => {
            return Err(ErrorKind::ConfigurationError(format!(
                "Could not parse strict spec (should be boolean) {:?}",
                v
            ))
            .into());
        }
    };


    let ports = match toml.get("ports") {
        Some(&Value::Array(ref ports)) => {
            let mut pouts = Vec::with_capacity(ports.len());
            for port in ports {
                let p = try!(read_port(port));
                pouts.push(p);
                // match read_port(port) {
            }
            pouts
        }
        None => Vec::with_capacity(0),
        _ => {
            error!("Ports is not an array");
            return Err(ErrorKind::ConfigurationError(String::from("Ports is not an array")).into());
        }
    };

    let vdevs = match toml.get("vdev") {
        Some(&Value::Array(ref vdevs)) => {
            let mut vouts = Vec::with_capacity(vdevs.len());
            for vdev in vdevs {
                vouts.push(vdev.as_str().unwrap().to_string());
            }
            vouts
        }
        None => Vec::with_capacity(0),
        _ => {
            error!("Could not parse vdev");
            return Err(ErrorKind::ConfigurationError(String::from("Could not parse vdev")).into());
        }
    };

    Ok(NetbricksConfiguration {
        name,
        primary_core: master_lcore,
        cores,
        strict,
        secondary,
        pool_size,
        cache_size,
        ports,
        vdevs,
        mbuf_cnt,
    })
}

/// Read a configuration file and create a `NetbricksConfiguration` structure.
/// `filename` should be TOML formatted file.
pub fn read_configuration(filename: &str) -> errors::Result<NetbricksConfiguration> {
    let mut toml_str = String::new();
    File::open(filename)
        .and_then(|mut f| f.read_to_string(&mut toml_str))
        .unwrap();
    read_configuration_from_str(&toml_str[..], filename)
}

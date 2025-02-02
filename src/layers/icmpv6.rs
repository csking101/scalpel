//! ICMPv6 Datagram

use std::convert::TryInto;

use serde::Serialize;

use crate::errors::Error;
use crate::layers::ipv6;
use crate::types::IPv6Address;
use crate::Layer;

/// IANA Assigned protocol number for ICMP
pub const IPPROTO_ICMPV6: u8 = 58_u8;
/// ICMP header length
pub const ICMPV6_HEADER_LENGTH: usize = 8_usize;

// Register ICMPv6 with Protocol Handler in IPv6
pub(crate) fn register_defaults() -> Result<(), Error> {
    ipv6::register_next_header(IPPROTO_ICMPV6, ICMPv6::creator)
}

// defining the types
pub const ICMPV6_DESTINATION_UNREACHABLE: u8 = 1;
pub const ICMPV6_PACKET_TOO_BIG: u8 = 2;
pub const ICMPV6_TIME_EXCEEDED: u8 = 3;
pub const ICMPV6_ECHO_REQUEST: u8 = 128;
pub const ICMPV6_ECHO_REPLY: u8 = 129;
pub const ICMPV6_ROUTER_SOLICITATION: u8 = 133;
pub const ICMPV6_ROUTER_ADVERTISEMENT: u8 = 134;
pub const ICMPV6_NEIGHBOR_SOLICITATION: u8 = 135;
pub const ICMPV6_NEIGHBOR_ADVERTISEMENT: u8 = 136;
pub const ICMPV6_REDIRECT: u8 = 137;

#[derive(Default, Debug, Serialize)]
#[serde(untagged)]
pub enum Icmpv6Type {
    #[default]
    Empty,
    Unsupported(Icmpv6Unsupported),
    #[serde[rename = "unsupported"]]
    EchoRequest(Icmpv6Echo),
    EchoReply(Icmpv6Echo),
    PacketSizeTooBig(Icmpv6PacketSizeTooBig),
    RouterAdvertisement(Icmpv6RouterAdvertisement),
    NeighborSolicitation(Icmpv6NeighborSolicitation),
    NeighborAdvertisement(Icmpv6NeighborAdvertisement),
    Redirect(Icmpv6Redirect),
}

#[derive(Debug, Default, Serialize, Copy, Clone)]
pub struct RouterAdvFlags {
    managed_address_flag: bool,
    other_config: bool, //This is only 1 bit
}

#[derive(Debug, Default, Serialize)]
pub struct NeighborAdvFlags {
    from_router: bool,
    solicited_flag: bool,
    override_flag: bool,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6Echo {
    identifier: u16,
    sequence_number: u16,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6PacketSizeTooBig {
    mtu: u32,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6Unsupported {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "hex::serde::serialize"
    )]
    unsupported: Vec<u8>,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6RouterAdvertisement {
    cur_hop_limit: u8,
    flags: RouterAdvFlags,
    router_lifetime: u16,
    reachable_time: u32,
    retrans_timer: u32,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6NeighborSolicitation {
    target_address: IPv6Address,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6NeighborAdvertisement {
    flags: NeighborAdvFlags,
    target_address: IPv6Address,
}

#[derive(Default, Debug, Serialize)]
pub struct Icmpv6Redirect {
    target_address: IPv6Address,
    destination_address: IPv6Address,
}

/// Structure representing the ICMPv6 Header
#[derive(Default, Debug, Serialize)]
pub struct ICMPv6 {
    #[serde(rename = "type")]
    icmp_type: u8,
    code: u8,
    #[serde(serialize_with = "crate::types::hex::serialize_lower_hex_u16")]
    checksum: u16,
    #[serde(flatten)]
    rest_of_header: Icmpv6Type,
}

impl ICMPv6 {
    pub(crate) fn creator() -> Box<dyn Layer + Send> {
        Box::<ICMPv6>::default()
    }
}

impl Layer for ICMPv6 {
    fn decode_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<(Option<Box<dyn Layer + Send>>, usize), Error> {
        let decoded;

        if bytes.len() < ICMPV6_HEADER_LENGTH {
            return Err(Error::TooShort {
                required: ICMPV6_HEADER_LENGTH,
                available: bytes.len(),
                data: hex::encode(bytes),
            });
        }

        // decode type, code and checksum
        self.icmp_type = u8::from_be(bytes[0]);
        self.code = u8::from_be(bytes[1]);
        self.checksum = (bytes[2] as u16) << 8 | (bytes[3] as u16);
        self.rest_of_header = match self.icmp_type {
            ICMPV6_ECHO_REQUEST => {
                let identifier = (bytes[4] as u16) << 8 | (bytes[5] as u16);
                let sequence_number = (bytes[6] as u16) << 8 | (bytes[7] as u16);
                decoded = 8;
                Icmpv6Type::EchoReply(Icmpv6Echo {
                    identifier,
                    sequence_number,
                })
            }

            ICMPV6_ECHO_REPLY => {
                let identifier = (bytes[4] as u16) << 8 | (bytes[5] as u16);
                let sequence_number = (bytes[6] as u16) << 8 | (bytes[7] as u16);
                decoded = 8;
                Icmpv6Type::EchoReply(Icmpv6Echo {
                    identifier,
                    sequence_number,
                })
            }

            ICMPV6_PACKET_TOO_BIG => {
                let mtu = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
                decoded = 8;
                Icmpv6Type::PacketSizeTooBig(Icmpv6PacketSizeTooBig { mtu })
            }

            ICMPV6_DESTINATION_UNREACHABLE | ICMPV6_TIME_EXCEEDED => {
                decoded = 8;
                Icmpv6Type::Empty
            }

            ICMPV6_ROUTER_ADVERTISEMENT => {
                let mut flags = RouterAdvFlags::default();
                let cur_hop_limit = u8::from_be(bytes[4]);
                flags.managed_address_flag = ((bytes[5] >> 7) & 0x01) == 0x01;
                flags.other_config = ((bytes[5] >> 6) & 0x01) == 0x01;

                let router_lifetime: u16 = (bytes[6] as u16) << 8 | (bytes[7] as u16);
                let reachable_time: u32 = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
                let retrans_timer: u32 = u32::from_be_bytes(bytes[12..16].try_into().unwrap());
                decoded = 16;
                Icmpv6Type::RouterAdvertisement(Icmpv6RouterAdvertisement {
                    cur_hop_limit,
                    flags,
                    router_lifetime,
                    reachable_time,
                    retrans_timer,
                })
            }

            ICMPV6_NEIGHBOR_SOLICITATION => {
                let target_address = bytes[8..24].try_into().unwrap();
                decoded = 24;
                Icmpv6Type::NeighborSolicitation(Icmpv6NeighborSolicitation { target_address })
            }

            ICMPV6_NEIGHBOR_ADVERTISEMENT => {
                let mut flags = NeighborAdvFlags::default();
                flags.from_router = ((bytes[4] >> 7) & 0x01) == 0x01;
                flags.solicited_flag = ((bytes[4] >> 6) & 0x01) == 0x01;
                flags.override_flag = ((bytes[4] >> 5) & 0x01) == 0x01;
                let target_address = bytes[8..24].try_into().unwrap();
                decoded = 24;
                Icmpv6Type::NeighborAdvertisement(Icmpv6NeighborAdvertisement {
                    flags,
                    target_address,
                })
            }
            ICMPV6_REDIRECT => {
                let target_address = bytes[8..24].try_into().unwrap();
                let destination_address = bytes[24..40].try_into().unwrap();
                decoded = 40;
                Icmpv6Type::Redirect(Icmpv6Redirect {
                    target_address,
                    destination_address,
                })
            }

            _ => {
                decoded = bytes.len();
                Icmpv6Type::Unsupported(Icmpv6Unsupported {
                    unsupported: bytes[4..].to_vec(),
                })
            }
        };
        Ok((None, decoded))
    }
    fn name(&self) -> &'static str {
        "ICMPV6"
    }

    fn short_name(&self) -> &'static str {
        "icmpv6"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::layers;
    use crate::{Layer, Packet, ENCAP_TYPE_ETH};

    #[test]
    fn parse_valid_icmpv6_packet() {
        let _ = layers::register_defaults();

        let icmpv6_packet = vec![
            0x00, 0x50, 0x56, 0x8a, 0x22, 0x80, 0x00, 0x50, 0x56, 0x8a, 0x0f, 0xe9, 0x86, 0xdd,
            0x60, 0x00, 0x00, 0x00, 0x00, 0x40, 0x3a, 0x40, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x20, 0x01, 0x05, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x80, 0x00,
            0xc9, 0x2a, 0x0e, 0x20, 0x00, 0x01, 0x1c, 0xcc, 0x53, 0x4f, 0x00, 0x00, 0x00, 0x00,
            0x1e, 0x03, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
            0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23,
            0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31,
            0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
        ];

        let mut icmpv6: Box<dyn Layer> = Box::new(super::ICMPv6::default());

        let p = icmpv6.decode_bytes(&icmpv6_packet[34..]);
        assert!(p.is_ok(), "{:#?}", icmpv6);
    }

    #[test]
    fn test_icmpv6_parse_regression() {
        let _ = layers::register_defaults();

        let test_icmpv6_packet = vec![
            0x00, 0x50, 0x56, 0x8a, 0x22, 0x80, 0x00, 0x50, 0x56, 0x8a, 0x0f, 0xe9, 0x86, 0xdd,
            0x60, 0x00, 0x00, 0x00, 0x00, 0x40, 0x3a, 0x40, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x20, 0x01, 0x05, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x80, 0x00,
            0xc9, 0x2a, 0x0e, 0x20, 0x00, 0x01, 0x1c, 0xcc, 0x53, 0x4f, 0x00, 0x00, 0x00, 0x00,
            0x1e, 0x03, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
            0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23,
            0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31,
            0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
        ];

        let p = Packet::from_bytes(&test_icmpv6_packet, ENCAP_TYPE_ETH);
        assert!(p.is_ok());
        let p = p.unwrap();
        assert!(p.layers.len() == 3, "{:#?}", p);
    }

    #[test]
    fn parse_icmpv6_redirect_packet() {
        let _ = layers::register_defaults();

        let icmpv6_echo_request_packet = vec![
            0x00, 0x0c, 0x29, 0x23, 0x16, 0x87, 0x00, 0x0c, 0x29, 0x25, 0xcf, 0xa1, 0x86, 0xdd,
            0x6e, 0x00, 0x00, 0x00, 0x00, 0xa0, 0x3a, 0xff, 0xfe, 0x80, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x0c, 0x29, 0xff, 0xfe, 0x25, 0xcf, 0xa1, 0x20, 0x01, 0x0d, 0xb8,
            0x00, 0x01, 0x00, 0x00, 0x99, 0x77, 0xf3, 0x9e, 0x80, 0xcb, 0x4e, 0xa6, 0x89, 0x00,
            0xc0, 0x7e, 0x00, 0x00, 0x00, 0x00, 0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x0c, 0x29, 0xff, 0xfe, 0xfc, 0x2c, 0x3b, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x02,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x01, 0x00, 0x0c,
            0x29, 0xfc, 0x2c, 0x3b, 0x04, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x60, 0x08,
            0xb9, 0x2f, 0x00, 0x40, 0x3a, 0x3f, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 0x00, 0x00,
            0x99, 0x77, 0xf3, 0x9e, 0x80, 0xcb, 0x4e, 0xa6, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x02,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0xe7, 0x43,
            0x1d, 0x85, 0x00, 0x01, 0x72, 0x6b, 0xf8, 0x5c, 0x00, 0x00, 0x00, 0x00, 0x91, 0x21,
            0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
            0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25,
            0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x32, 0x33,
            0x34, 0x35, 0x36, 0x37,
        ];

        let p = Packet::from_bytes(&icmpv6_echo_request_packet, ENCAP_TYPE_ETH);
        assert!(p.is_ok());
        let p = p.unwrap();
        let icmpv6_packet = serde_json::to_value(&p.layers[2]).unwrap();
        assert_eq!(icmpv6_packet.get("type"), Some(&json!(137)));
        assert_eq!(icmpv6_packet.get("code"), Some(&json!(0)));
        assert_eq!(icmpv6_packet.get("checksum"), Some(&json!("0xc07e")));
        assert_eq!(
            icmpv6_packet.get("target_address"),
            Some(&json!("fe80::20c:29ff:fefc:2c3b"))
        );
        assert_eq!(
            icmpv6_packet.get("destination_address"),
            Some(&json!("2001:db8:2::1"))
        );
    }

    #[test]
    fn icmpv6_echo_request_packet() {
        let _ = layers::register_defaults();

        let icmpv6_echo_request_packet = vec![
            0x00, 0x50, 0x56, 0x8a, 0x22, 0x80, 0x00, 0x50, 0x56, 0x8a, 0x0f, 0xe9, 0x86, 0xdd,
            0x60, 0x00, 0x00, 0x00, 0x00, 0x40, 0x3a, 0x40, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x20, 0x01, 0x05, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x80, 0x00,
            0xc9, 0x2a, 0x0e, 0x20, 0x00, 0x01, 0x1c, 0xcc, 0x53, 0x4f, 0x00, 0x00, 0x00, 0x00,
            0x1e, 0x03, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
            0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23,
            0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31,
            0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
        ];

        let p = Packet::from_bytes(&icmpv6_echo_request_packet, ENCAP_TYPE_ETH);
        assert!(p.is_ok());
        let p = p.unwrap();

        let icmpv6_packet = serde_json::to_value(&p.layers[2]).unwrap();
        assert_eq!(icmpv6_packet.get("type"), Some(&json!(128)));
        assert_eq!(icmpv6_packet.get("code"), Some(&json!(0)));
        assert_eq!(icmpv6_packet.get("checksum"), Some(&json!("0xc92a")));
        assert_eq!(icmpv6_packet.get("identifier"), Some(&json!(3616)));
        assert_eq!(icmpv6_packet.get("sequence_number"), Some(&json!(1)));
    }

    #[test]
    fn parse_icmpv6_neighbor_solicitation_packet() {
        let _ = layers::register_defaults();

        let icmpv6_neighbor_solicitation_packet = vec![
            0x33, 0x33, 0xff, 0x00, 0x00, 0x25, 0x00, 0x50, 0x56, 0x8a, 0x0f, 0xe9, 0x86, 0xdd,
            0x60, 0x00, 0x00, 0x00, 0x00, 0x20, 0x3a, 0xff, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0xff, 0x02, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xff, 0x00, 0x00, 0x25, 0x87, 0x00,
            0xc6, 0x6e, 0x00, 0x00, 0x00, 0x00, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x01, 0x01, 0x00, 0x50, 0x56, 0x8a,
            0x0f, 0xe9,
        ];

        let p = Packet::from_bytes(&icmpv6_neighbor_solicitation_packet, ENCAP_TYPE_ETH);
        assert!(p.is_ok());
        let p = p.unwrap();

        let icmpv6_packet = serde_json::to_value(&p.layers[2]).unwrap();
        assert_eq!(icmpv6_packet.get("type"), Some(&json!(135)));
        assert_eq!(icmpv6_packet.get("code"), Some(&json!(0)));
        assert_eq!(icmpv6_packet.get("checksum"), Some(&json!("0xc66e")));
        assert_eq!(
            icmpv6_packet.get("target_address"),
            Some(&json!("2001:500:100::25"))
        );
    }

    #[test]
    fn parse_unsupported_icmp_type() {
        let _ = layers::register_defaults();

        let icmpv6_destination_unreachable_packet = vec![
            0x00, 0x50, 0x56, 0x8a, 0x0f, 0xe9, 0x00, 0x50, 0x56, 0x8a, 0x22, 0x80, 0x86, 0xdd,
            0x60, 0x00, 0x00, 0x00, 0x00, 0x20, 0x3a, 0xff, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x20, 0x01, 0x05, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x10, 0x00,
            0x29, 0xdb, 0x60, 0x00, 0x00, 0x00, 0x20, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x02, 0x01, 0x00, 0x50, 0x56, 0x8a,
            0x22, 0x80,
        ];
        let p = Packet::from_bytes(&icmpv6_destination_unreachable_packet, ENCAP_TYPE_ETH);
        assert!(p.is_ok());
        let p = p.unwrap();

        let icmpv6_packet = serde_json::to_value(&p.layers[2]).unwrap();
        assert_eq!(icmpv6_packet.get("type"), Some(&json!(16)));
        assert_eq!(icmpv6_packet.get("code"), Some(&json!(0)));
        assert_eq!(icmpv6_packet.get("checksum"), Some(&json!("0x29db")));
        assert_eq!(
            icmpv6_packet.get("unsupported"),
            Some(&json!(
                "600000002001050001000000000000000000002502010050568a2280"
            ))
        );
    }
}

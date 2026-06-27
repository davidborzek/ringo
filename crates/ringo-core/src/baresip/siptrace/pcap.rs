//! Minimal libpcap writer — transport- and protocol-agnostic, knows nothing of
//! SIP. Frames an arbitrary payload as one Ethernet + IPv4/IPv6 + UDP datagram
//! and wraps it in a pcap record, so tools like sngrep / Wireshark can parse a
//! flow even when the real transport was TLS (we only ever see the plaintext).
//!
//! Link type EN10MB (Ethernet) for maximum tool compatibility; we frame even
//! TCP/TLS as UDP — the real transport is cosmetic for parsing, and one
//! datagram per message gives the tools a clean ladder.

use std::time::{SystemTime, UNIX_EPOCH};

const ETHERTYPE_IPV4: [u8; 2] = [0x08, 0x00];
const ETHERTYPE_IPV6: [u8; 2] = [0x86, 0xdd];
const IP_PROTO_UDP: u8 = 17;
const IPV4_VERSION_IHL: u8 = 0x45; // IPv4, 5 × 32-bit words (no options)
const IPV4_FLAG_DONT_FRAGMENT: u16 = 0x4000;
const IPV6_VERSION: u8 = 0x60; // version nibble 6, traffic class 0
const DEFAULT_HOP_LIMIT: u8 = 64; // IPv4 TTL / IPv6 hop limit

const ETH_HEADER_LEN: usize = 14;
const IPV4_HEADER_LEN: usize = 20;
const UDP_HEADER_LEN: usize = 8;

/// pcap global header (24 bytes): little-endian magic, v2.4, Ethernet link type.
pub fn global_header() -> [u8; 24] {
    let mut h = [0u8; 24];
    h[0..4].copy_from_slice(&0xa1b2_c3d4u32.to_le_bytes()); // magic
    h[4..6].copy_from_slice(&2u16.to_le_bytes()); // version major
    h[6..8].copy_from_slice(&4u16.to_le_bytes()); // version minor
    // thiszone (4) + sigfigs (4) = 0
    h[16..20].copy_from_slice(&262_144u32.to_le_bytes()); // snaplen
    h[20..24].copy_from_slice(&1u32.to_le_bytes()); // network = LINKTYPE_ETHERNET
    h
}

/// One pcap record: 16-byte record header + Ethernet/IP/UDP framing of
/// `payload`. `src`/`dst` are 4 bytes (IPv4) or 16 (IPv6), in network order.
pub fn record(
    src: &[u8],
    dst: &[u8],
    sport: u16,
    dport: u16,
    payload: &[u8],
    ts: SystemTime,
) -> Vec<u8> {
    let pkt = eth_ip_udp(src, dst, sport, dport, payload);
    let now = ts.duration_since(UNIX_EPOCH).unwrap_or_default();

    let mut rec = Vec::with_capacity(16 + pkt.len());
    rec.extend((now.as_secs() as u32).to_le_bytes()); // ts_sec
    rec.extend(now.subsec_micros().to_le_bytes()); // ts_usec
    rec.extend((pkt.len() as u32).to_le_bytes()); // incl_len
    rec.extend((pkt.len() as u32).to_le_bytes()); // orig_len
    rec.extend_from_slice(&pkt);
    rec
}

/// Frame `payload` as one Ethernet + IPv4/IPv6 + UDP datagram. `src`/`dst` are
/// 4 bytes (IPv4) or 16 (IPv6), in network order.
fn eth_ip_udp(src: &[u8], dst: &[u8], sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let udp_len = (UDP_HEADER_LEN + payload.len()) as u16;

    // Ethernet header: zeroed MACs + ethertype.
    let mut pkt = Vec::with_capacity(ETH_HEADER_LEN + IPV4_HEADER_LEN + udp_len as usize);
    pkt.extend([0u8; 12]); // dst + src MAC
    pkt.extend(if src.len() == 4 {
        ETHERTYPE_IPV4
    } else {
        ETHERTYPE_IPV6
    });

    // IP header, fields in wire order.
    if src.len() == 4 {
        let total_len = (IPV4_HEADER_LEN as u16) + udp_len;
        let mut ip = Vec::with_capacity(IPV4_HEADER_LEN);
        ip.push(IPV4_VERSION_IHL);
        ip.push(0); // DSCP / ECN
        ip.extend(total_len.to_be_bytes());
        ip.extend(0u16.to_be_bytes()); // identification
        ip.extend(IPV4_FLAG_DONT_FRAGMENT.to_be_bytes()); // flags + fragment offset
        ip.push(DEFAULT_HOP_LIMIT); // TTL
        ip.push(IP_PROTO_UDP);
        ip.extend(0u16.to_be_bytes()); // checksum placeholder (patched below)
        ip.extend_from_slice(src);
        ip.extend_from_slice(dst);
        let csum = ipv4_checksum(&ip);
        ip[10..12].copy_from_slice(&csum.to_be_bytes());
        pkt.extend_from_slice(&ip);
    } else {
        pkt.push(IPV6_VERSION);
        pkt.extend([0u8; 3]); // traffic class + flow label
        pkt.extend(udp_len.to_be_bytes()); // payload length
        pkt.push(IP_PROTO_UDP); // next header
        pkt.push(DEFAULT_HOP_LIMIT); // hop limit
        pkt.extend_from_slice(src);
        pkt.extend_from_slice(dst);
    }

    // UDP header + payload.
    pkt.extend(sport.to_be_bytes());
    pkt.extend(dport.to_be_bytes());
    pkt.extend(udp_len.to_be_bytes());
    pkt.extend(0u16.to_be_bytes()); // checksum (0 = not computed)
    pkt.extend_from_slice(payload);
    pkt
}

/// Standard IPv4 header checksum: one's-complement sum of the header's 16-bit
/// words (the checksum field itself must be zero while summing).
fn ipv4_checksum(hdr: &[u8]) -> u16 {
    let mut sum = 0u32;
    for w in hdr.chunks_exact(2) {
        sum += u16::from_be_bytes([w[0], w[1]]) as u32;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP: usize = ETH_HEADER_LEN; // start of IP header
    const UDP: usize = ETH_HEADER_LEN + IPV4_HEADER_LEN; // start of UDP (IPv4)

    #[test]
    fn ipv4_frame_is_well_formed() {
        let sip = b"OPTIONS sip:b@example.com SIP/2.0\r\n\
                    Via: SIP/2.0/UDP 10.0.0.1:5060\r\n\
                    From: <sip:a@example.com>;tag=1\r\n\
                    To: <sip:b@example.com>\r\n\
                    Call-ID: selftest@10.0.0.1\r\n\
                    CSeq: 1 OPTIONS\r\n\r\n";
        let pkt = eth_ip_udp(&[10, 0, 0, 1], &[10, 0, 0, 2], 5060, 5061, sip);
        assert_eq!(&pkt[12..14], &ETHERTYPE_IPV4, "ethertype IPv4");
        assert_eq!(pkt[IP], IPV4_VERSION_IHL, "IPv4 version/IHL");
        assert_eq!(pkt[IP + 9], IP_PROTO_UDP, "IP proto UDP");
        assert_eq!(&pkt[UDP..UDP + 2], &5060u16.to_be_bytes(), "src port");
        assert_eq!(&pkt[UDP + 2..UDP + 4], &5061u16.to_be_bytes(), "dst port");
        assert_eq!(&pkt[UDP + UDP_HEADER_LEN..], sip, "SIP payload intact");
        // Re-summing the whole header (checksum included) must yield 0xffff.
        let mut sum = 0u32;
        for w in pkt[IP..IP + IPV4_HEADER_LEN].chunks_exact(2) {
            sum += u16::from_be_bytes([w[0], w[1]]) as u32;
        }
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        assert_eq!(sum as u16, 0xffff, "IPv4 checksum valid");
    }

    #[test]
    fn ipv6_frame_is_well_formed() {
        const V6_HEADER_LEN: usize = 40;
        let pkt = eth_ip_udp(&[0u8; 16], &[1u8; 16], 5060, 5060, b"PING");
        assert_eq!(&pkt[12..14], &ETHERTYPE_IPV6, "ethertype IPv6");
        assert_eq!(pkt[IP], IPV6_VERSION, "IPv6 version");
        assert_eq!(pkt[IP + 6], IP_PROTO_UDP, "IPv6 next header UDP");
        let payload = ETH_HEADER_LEN + V6_HEADER_LEN + UDP_HEADER_LEN;
        assert_eq!(&pkt[payload..], b"PING", "payload after v6+udp");
    }

    #[test]
    fn global_header_magic_and_linktype() {
        let gh = global_header();
        assert_eq!(&gh[0..4], &0xa1b2_c3d4u32.to_le_bytes(), "pcap magic");
        assert_eq!(&gh[20..24], &1u32.to_le_bytes(), "LINKTYPE_ETHERNET");
    }
}

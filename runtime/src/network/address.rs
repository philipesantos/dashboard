//! Which addresses a connector may reach (`specs/003-plugin-connectors/research.md`, R6): only
//! public ones, judged from an explicit table of the IANA IPv4 and IPv6 Special-Purpose Address
//! Registries, so a reviewer can check every row. `std`'s `is_global` is unstable.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Whether `address` is a public address of the internet, the only kind a connector reaches
/// outside development. An IPv6 address that embeds an IPv4 one is judged by the IPv4 address.
pub(crate) fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_v4(address),
        IpAddr::V6(address) => match embedded_v4(address) {
            Some(embedded) => is_public_v4(embedded),
            None => is_public_v6(address),
        },
    }
}

/// Whether `address` is this machine: what a local connector reaches in development (FR-018).
pub(crate) fn is_loopback(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => address.is_loopback(),
        IpAddr::V6(address) => {
            address.is_loopback() || address.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// IPv4 ranges that are not public: the IANA IPv4 Special-Purpose Address Registry rows that are
/// not globally reachable, plus multicast. Each is `(network, prefix length)`.
const NOT_PUBLIC_V4: &[(Ipv4Addr, u8)] = &[
    // "This network" (RFC 791), including 0.0.0.0, the unspecified address
    (Ipv4Addr::new(0, 0, 0, 0), 8),
    // Private-Use (RFC 1918)
    (Ipv4Addr::new(10, 0, 0, 0), 8),
    // Shared Address Space, carrier-grade NAT (RFC 6598)
    (Ipv4Addr::new(100, 64, 0, 0), 10),
    // Loopback (RFC 1122)
    (Ipv4Addr::new(127, 0, 0, 0), 8),
    // Link Local (RFC 3927), cloud metadata services included
    (Ipv4Addr::new(169, 254, 0, 0), 16),
    // Private-Use (RFC 1918)
    (Ipv4Addr::new(172, 16, 0, 0), 12),
    // IETF Protocol Assignments (RFC 6890)
    (Ipv4Addr::new(192, 0, 0, 0), 24),
    // Documentation, TEST-NET-1 (RFC 5737)
    (Ipv4Addr::new(192, 0, 2, 0), 24),
    // Deprecated 6to4 Relay Anycast (RFC 7526)
    (Ipv4Addr::new(192, 88, 99, 0), 24),
    // Private-Use (RFC 1918)
    (Ipv4Addr::new(192, 168, 0, 0), 16),
    // Benchmarking (RFC 2544)
    (Ipv4Addr::new(198, 18, 0, 0), 15),
    // Documentation, TEST-NET-2 (RFC 5737)
    (Ipv4Addr::new(198, 51, 100, 0), 24),
    // Documentation, TEST-NET-3 (RFC 5737)
    (Ipv4Addr::new(203, 0, 113, 0), 24),
    // Multicast (RFC 5771)
    (Ipv4Addr::new(224, 0, 0, 0), 4),
    // Reserved (RFC 1112), and Limited Broadcast, 255.255.255.255 (RFC 919)
    (Ipv4Addr::new(240, 0, 0, 0), 4),
];

/// IPv6 ranges inside global unicast (`2000::/3`) that are not public, from the IANA IPv6
/// Special-Purpose Address Registry. Everything outside `2000::/3` is not public either: the
/// unspecified and loopback addresses, discard-only (`100::/64`), local-use translation
/// (`64:ff9b:1::/48`), unique local (`fc00::/7`), link-local (`fe80::/10`) and multicast
/// (`ff00::/8`) all lie there.
const NOT_PUBLIC_V6: &[(Ipv6Addr, u8)] = &[
    // IETF Protocol Assignments (RFC 2928), taken whole: Teredo (2001::/32), benchmarking,
    // ORCHID and the anycast rows under it; no service a connector reaches lives there
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23),
    // Documentation (RFC 3849)
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    // Documentation (RFC 9637)
    (Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20),
    // Segment Routing SIDs (RFC 9602)
    (Ipv6Addr::new(0x5f00, 0, 0, 0, 0, 0, 0, 0), 16),
];

fn is_public_v4(address: Ipv4Addr) -> bool {
    let bits = u32::from(address);
    !NOT_PUBLIC_V4.iter().any(|(network, prefix)| {
        let mask = u32::MAX.checked_shl(32 - u32::from(*prefix)).unwrap_or(0);
        bits & mask == u32::from(*network) & mask
    })
}

fn is_public_v6(address: Ipv6Addr) -> bool {
    let bits = u128::from(address);
    let in_range = |network: Ipv6Addr, prefix: u8| {
        let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
        bits & mask == u128::from(network) & mask
    };
    in_range(Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3)
        && !NOT_PUBLIC_V6
            .iter()
            .any(|(network, prefix)| in_range(*network, *prefix))
}

/// The IPv4 address an IPv6 form carries, judged in its place: IPv4-mapped (`::ffff:0:0/96`),
/// IPv4-compatible (`::/96`, deprecated, but routed by some stacks), NAT64 (`64:ff9b::/96`,
/// RFC 6052) and 6to4 (`2002::/16`, RFC 3056).
fn embedded_v4(address: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return Some(mapped);
    }
    let segments = address.segments();
    let last = |high: u16, low: u16| Ipv4Addr::from((u32::from(high) << 16) | u32::from(low));
    match segments {
        // :: and ::1 are judged as IPv6
        [0, 0, 0, 0, 0, 0, 0, 0 | 1] => None,
        [0, 0, 0, 0, 0, 0, high, low] => Some(last(high, low)),
        [0x64, 0xff9b, 0, 0, 0, 0, high, low] => Some(last(high, low)),
        [0x2002, high, low, ..] => Some(last(high, low)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(text: &str) -> IpAddr {
        IpAddr::V4(text.parse::<Ipv4Addr>().unwrap())
    }

    fn v6(text: &str) -> IpAddr {
        IpAddr::V6(text.parse::<Ipv6Addr>().unwrap())
    }

    #[track_caller]
    fn assert_not_public(addresses: &[&str]) {
        for address in addresses {
            let ip: IpAddr = address.parse().unwrap();
            assert!(!is_public(ip), "{address} should not be public");
        }
    }

    #[test]
    fn unspecified_and_this_network() {
        assert_not_public(&["0.0.0.0", "0.1.2.3", "0.255.255.255", "::"]);
    }

    #[test]
    fn loopback() {
        assert_not_public(&["127.0.0.1", "127.255.255.254", "::1"]);
    }

    #[test]
    fn private_ranges() {
        assert_not_public(&[
            "10.0.0.1",
            "10.255.255.255",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "192.168.255.255",
        ]);
        assert!(is_public(v4("172.15.255.255")));
        assert!(is_public(v4("172.32.0.0")));
    }

    #[test]
    fn shared_address_space() {
        assert_not_public(&["100.64.0.1", "100.127.255.255"]);
        assert!(is_public(v4("100.63.255.255")));
        assert!(is_public(v4("100.128.0.0")));
    }

    #[test]
    fn link_local() {
        assert_not_public(&["169.254.0.1", "169.254.169.254", "fe80::1", "febf::1"]);
    }

    #[test]
    fn ietf_protocol_assignments() {
        assert_not_public(&["192.0.0.1", "192.0.0.255"]);
    }

    #[test]
    fn documentation_ranges() {
        assert_not_public(&[
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "2001:db8::1",
            "2001:db8:ffff::1",
        ]);
    }

    #[test]
    fn benchmarking() {
        assert_not_public(&["198.18.0.1", "198.19.255.255"]);
        assert!(is_public(v4("198.20.0.0")));
    }

    #[test]
    fn multicast_reserved_and_broadcast() {
        assert_not_public(&[
            "224.0.0.1",
            "239.255.255.255",
            "240.0.0.1",
            "255.255.255.254",
            "255.255.255.255",
            "ff02::1",
            "ff00::",
        ]);
    }

    #[test]
    fn unique_local() {
        assert_not_public(&["fc00::1", "fd12:3456::1", "fdff:ffff::1"]);
    }

    #[test]
    fn ipv6_forms_that_embed_an_ipv4_address_are_judged_by_it() {
        assert_not_public(&[
            "::ffff:127.0.0.1",
            "::ffff:192.168.0.1",
            "::ffff:10.0.0.1",
            "::127.0.0.1",
            "::10.0.0.1",
            "64:ff9b::7f00:1",
            "64:ff9b::a00:1",
            "2002:7f00:1::",
            "2002:c0a8:1::",
        ]);
        // Teredo embeds its client's address obfuscated; the whole prefix is not public
        assert_not_public(&["2001::1", "2001:0:4136:e378:8000:63bf:3fff:fdd2"]);
        // An embedded public address is public
        assert!(is_public(v6("::ffff:8.8.8.8")));
        assert!(is_public(v6("64:ff9b::808:808")));
        assert!(is_public(v6("2002:808:808::1")));
    }

    #[test]
    fn other_special_ipv6_ranges() {
        // Discard-only, the IETF protocol block and the ORCHID ranges
        assert_not_public(&["100::1", "2001:10::1", "2001:20::1"]);
    }

    #[test]
    fn public_addresses() {
        for address in ["8.8.8.8", "1.1.1.1", "140.82.112.3", "93.184.216.34"] {
            assert!(is_public(v4(address)), "{address}");
        }
        for address in [
            "2606:4700::1111",
            "2a00:1450:4001::200e",
            "2001:4860:4860::8888",
        ] {
            assert!(is_public(v6(address)), "{address}");
        }
    }

    #[test]
    fn loopback_is_only_this_machine() {
        assert!(is_loopback(v4("127.0.0.1")));
        assert!(is_loopback(v4("127.1.2.3")));
        assert!(is_loopback(v6("::1")));
        assert!(is_loopback(v6("::ffff:127.0.0.1")));
        for address in [
            "10.0.0.1",
            "192.168.1.10",
            "0.0.0.0",
            "169.254.1.1",
            "8.8.8.8",
        ] {
            assert!(!is_loopback(v4(address)), "{address}");
        }
        assert!(!is_loopback(v6("::")));
        assert!(!is_loopback(v6("fe80::1")));
    }
}

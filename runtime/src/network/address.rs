//! Which addresses a connector may reach (`specs/003-plugin-connectors/research.md`, R6): only
//! public ones, judged from an explicit table of the IANA IPv4 and IPv6 Special-Purpose Address
//! Registries, so a reviewer can check every row. `std`'s `is_global` is unstable.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Whether `address` is a public address of the internet, the only kind a connector reaches
/// outside development. An IPv6 address that embeds an IPv4 one is judged by the IPv4 address.
pub(crate) fn is_public(address: IpAddr) -> bool {
    let _ = address;
    true
}

/// Whether `address` is this machine: what a local connector reaches in development (FR-018).
pub(crate) fn is_loopback(address: IpAddr) -> bool {
    let _ = address;
    false
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

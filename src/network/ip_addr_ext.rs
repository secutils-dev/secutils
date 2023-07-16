use std::net::IpAddr;

pub trait IpAddrExt {
    fn is_global(&self) -> bool;
}

impl IpAddrExt for IpAddr {
    /// Copy of the Rust's `Ipv6Addr::is_global` unstable implementation. Should be removed as soon
    /// as standard API is stabilized in https://github.com/rust-lang/rust/issues/27709.
    fn is_global(&self) -> bool {
        if self.is_unspecified() || self.is_loopback() {
            return false;
        }

        match self {
            IpAddr::V4(ip) => {
                !(ip.octets()[0] == 0 // "This network"
                    || ip.is_private()
                    || ip.octets()[0] == 100 && (ip.octets()[1] & 0b1100_0000 == 0b0100_0000)
                    || ip.is_link_local()
                    // addresses reserved for future protocols (`192.0.0.0/24`)
                    ||(ip.octets()[0] == 192 && ip.octets()[1] == 0 && ip.octets()[2] == 0)
                    || ip.is_documentation()
                    || ip.octets()[0] == 198 && (ip.octets()[1] & 0xfe) == 18
                    || (ip.octets()[0] & 240 == 240 && !ip.is_broadcast())
                    || ip.is_broadcast())
            }
            IpAddr::V6(ip) => {
                !(
                    // IPv4-mapped Address (`::ffff:0:0/96`)
                    matches!(ip.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
                        // IPv4-IPv6 Translat. (`64:ff9b:1::/48`)
                        || matches!(ip.segments(), [0x64, 0xff9b, 1, _, _, _, _, _])
                        // Discard-Only Address Block (`100::/64`)
                        || matches!(ip.segments(), [0x100, 0, 0, 0, _, _, _, _])
                        // IETF Protocol Assignments (`2001::/23`)
                        || (matches!(ip.segments(), [0x2001, b, _, _, _, _, _, _] if b < 0x200)
                        && !(
                        // Port Control Protocol Anycast (`2001:1::1`)
                        u128::from_be_bytes(ip.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                            // Traversal Using Relays around NAT Anycast (`2001:1::2`)
                            || u128::from_be_bytes(ip.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                            // AMT (`2001:3::/32`)
                            || matches!(ip.segments(), [0x2001, 3, _, _, _, _, _, _])
                            // AS112-v6 (`2001:4:112::/48`)
                            || matches!(ip.segments(), [0x2001, 4, 0x112, _, _, _, _, _])
                            // ORCHIDv2 (`2001:20::/28`)
                            || matches!(ip.segments(), [0x2001, b, _, _, _, _, _, _] if (0x20..=0x2F).contains(&b))
                    ))
                        // is_documentation
                        || (ip.segments()[0] == 0x2001) && (ip.segments()[1] == 0xdb8)
                        // is_unique_local
                        || (ip.segments()[0] & 0xfe00) == 0xfc00
                        // is_unicast_link_local
                        ||  (ip.segments()[0] & 0xffc0) == 0xfe80
                )
            }
        }
    }
}

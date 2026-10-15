netdev 0.46.1 from crates.io, with two changes so that it compiles for iOS
(`libc` defines `in6_ifreq` and `ifmibdata` for macOS only):

- `src/os/darwin/ipv6_addr_flags.rs`: the ioctl is macOS-only; elsewhere the flags are the default.
- `src/stats/counters.rs`: the sysctl reading is macOS-only; on iOS there are no counters.

Used through `[patch.crates-io]` in `src-tauri/Cargo.toml`. Delete this directory and that
entry when a netdev release builds for iOS (0.46.1, 0.46.2 and 0.46.3 do not).
Licence: see LICENSE.

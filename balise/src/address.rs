use serde::{Deserialize, Serialize};
use std::{fmt, fmt::Formatter, io, str::FromStr};
pub use url::Host;

/// An endpoint address
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct Address {
    /// The hostname (either dns name or ip address)
    pub host: Host,
    /// The port number
    pub port: u16,
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.host.fmt(f)?;
        f.write_str(":")?;
        f.write_str(&self.port.to_string())?;
        Ok(())
    }
}

impl FromStr for Address {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hostname_end = s.find(':').unwrap_or_else(|| s.len());
        let host = Host::parse(&s[0..hostname_end]).expect("could not parse domain");
        let port = s[hostname_end + 1..s.len()]
            .parse()
            .expect("could not parse port");

        Ok(Address { host, port })
    }
}

use super::types::NodeRole;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::net::{Ipv6Addr, SocketAddr};
use std::str::FromStr;

/// Environment variable that overrides `replication.master_address`.
pub const MASTER_ADDRESS_ENV: &str = "SYNAP_MASTER_ADDRESS";

/// Address of the master a replica connects to: `host:port`, where `host` is
/// an IP literal (IPv6 written as `[addr]`) or a DNS name.
///
/// The name is kept unresolved and handed to the connector on every attempt,
/// so a docker-compose or Kubernetes service name is looked up again on each
/// reconnect — a container's IP can change between restarts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterAddress {
    host: String,
    port: u16,
}

impl MasterAddress {
    /// Host part: an IP literal (IPv6 without brackets) or a DNS name.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Port part.
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl From<SocketAddr> for MasterAddress {
    fn from(addr: SocketAddr) -> Self {
        Self {
            host: addr.ip().to_string(),
            port: addr.port(),
        }
    }
}

impl FromStr for MasterAddress {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(addr) = s.parse::<SocketAddr>() {
            if addr.port() == 0 {
                return Err(format!("`{s}` has port 0; expected host:port"));
            }
            return Ok(addr.into());
        }

        let (host, port) = s
            .rsplit_once(':')
            .ok_or_else(|| format!("`{s}` has no port; expected host:port"))?;
        if host.contains(':') || host.starts_with('[') {
            return Err(format!(
                "`{s}` is not a valid address; write IPv6 hosts as [addr]:port"
            ));
        }
        let valid_host = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if !valid_host {
            return Err(format!("`{s}` has an invalid host; expected host:port"));
        }
        let port = match port.parse::<u16>() {
            Ok(p) if p != 0 => p,
            _ => return Err(format!("`{s}` has an invalid port; expected host:port")),
        };

        Ok(Self {
            host: host.to_string(),
            port,
        })
    }
}

impl fmt::Display for MasterAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.parse::<Ipv6Addr>().is_ok() {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

impl Serialize for MasterAddress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for MasterAddress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(|e| {
            serde::de::Error::custom(format!("invalid replication.master_address: {e}"))
        })
    }
}

/// Replication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    /// Enable replication
    pub enabled: bool,

    /// Node role (master, replica, standalone)
    pub role: NodeRole,

    /// Master address (for replica nodes): `host:port`, resolved on every
    /// connection attempt
    pub master_address: Option<MasterAddress>,

    /// Replica listen address (for master to send data)
    pub replica_listen_address: Option<SocketAddr>,

    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,

    /// Maximum lag threshold (ms) before alerting
    pub max_lag_ms: u64,

    /// Replication buffer size (KB)
    pub buffer_size_kb: usize,

    /// Automatic reconnect on disconnect
    pub auto_reconnect: bool,

    /// Reconnect delay in milliseconds
    pub reconnect_delay_ms: u64,

    /// Replica timeout in seconds (master marks replica dead)
    pub replica_timeout_secs: u64,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            role: NodeRole::Standalone,
            master_address: None,
            replica_listen_address: None,
            heartbeat_interval_ms: 1000, // 1 second heartbeat
            max_lag_ms: 10000,           // 10 seconds max lag
            buffer_size_kb: 256,         // 256KB buffer
            auto_reconnect: true,
            reconnect_delay_ms: 5000, // 5 seconds
            replica_timeout_secs: 30, // 30 seconds timeout
        }
    }
}

impl ReplicationConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        match self.role {
            NodeRole::Replica => {
                if self.master_address.is_none() {
                    return Err("Replica node requires master_address".to_string());
                }
            }
            NodeRole::Master => {
                if self.replica_listen_address.is_none() {
                    return Err("Master node requires replica_listen_address".to_string());
                }
            }
            NodeRole::Standalone => {
                if self.enabled {
                    return Err("Standalone node cannot have replication enabled".to_string());
                }
            }
        }

        Ok(())
    }

    /// Check if this node is a master
    pub fn is_master(&self) -> bool {
        self.enabled && self.role == NodeRole::Master
    }

    /// Check if this node is a replica
    pub fn is_replica(&self) -> bool {
        self.enabled && self.role == NodeRole::Replica
    }

    /// Apply the [`MASTER_ADDRESS_ENV`] override. `value` is the variable's
    /// value when it is set; a malformed value is an error naming the variable.
    pub fn apply_master_address_override(&mut self, value: Option<&str>) -> Result<(), String> {
        if let Some(raw) = value {
            let addr = raw
                .parse()
                .map_err(|e| format!("{MASTER_ADDRESS_ENV}: {e}"))?;
            self.master_address = Some(addr);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_default_config() {
        let config = ReplicationConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.role, NodeRole::Standalone);
    }

    #[test]
    fn test_master_validation() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = NodeRole::Master;

        // Should fail without listen address
        assert!(config.validate().is_err());

        // Should succeed with listen address
        config.replica_listen_address = Some(SocketAddr::from_str("0.0.0.0:15501").unwrap());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_replica_validation() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = NodeRole::Replica;

        // Should fail without master address
        assert!(config.validate().is_err());

        // Should succeed with master address
        config.master_address = Some(SocketAddr::from_str("127.0.0.1:15501").unwrap().into());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn master_address_accepts_ip_and_hostname() {
        let ip: MasterAddress = "10.0.0.10:15600".parse().unwrap();
        assert_eq!((ip.host(), ip.port()), ("10.0.0.10", 15600));

        let name: MasterAddress = "master:15600".parse().unwrap();
        assert_eq!((name.host(), name.port()), ("master", 15600));

        let fqdn: MasterAddress = "synap-master.prod.svc.cluster.local:15600".parse().unwrap();
        assert_eq!(fqdn.host(), "synap-master.prod.svc.cluster.local");

        let v6: MasterAddress = "[::1]:15600".parse().unwrap();
        assert_eq!((v6.host(), v6.port()), ("::1", 15600));
        assert_eq!(v6.to_string(), "[::1]:15600");
        assert_eq!(name.to_string(), "master:15600");
    }

    #[test]
    fn master_address_rejects_malformed_values() {
        for bad in [
            "master",
            "master:",
            ":15600",
            "master:0",
            "master:99999",
            "master:port",
            "::1:15600",
            "bad host:15600",
            "",
        ] {
            assert!(
                bad.parse::<MasterAddress>().is_err(),
                "`{bad}` should be rejected"
            );
        }
    }

    #[test]
    fn master_address_from_socket_addr_round_trips() {
        let sock: SocketAddr = "[2001:db8::1]:15600".parse().unwrap();
        let addr = MasterAddress::from(sock);
        assert_eq!(addr.to_string(), "[2001:db8::1]:15600");
        assert_eq!(addr.to_string().parse::<SocketAddr>().unwrap(), sock);
    }

    #[test]
    fn malformed_master_address_fails_config_load_naming_the_field() {
        let yaml = "enabled: true\nrole: replica\nmaster_address: \"master\"\n\
                    replica_listen_address: null\nheartbeat_interval_ms: 1000\n\
                    max_lag_ms: 10000\nbuffer_size_kb: 256\nauto_reconnect: true\n\
                    reconnect_delay_ms: 5000\nreplica_timeout_secs: 30\n";
        let err = serde_saphyr::from_str::<ReplicationConfig>(yaml)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("replication.master_address"),
            "error should name the field: {err}"
        );

        let ok = yaml.replace("\"master\"", "\"master:15600\"");
        let config: ReplicationConfig = serde_saphyr::from_str(&ok).unwrap();
        assert_eq!(config.master_address.unwrap().to_string(), "master:15600");
    }

    #[test]
    fn environment_override_wins_over_the_file() {
        let mut config = ReplicationConfig {
            enabled: true,
            role: NodeRole::Replica,
            master_address: Some("10.0.0.10:15600".parse().unwrap()),
            ..Default::default()
        };

        config.apply_master_address_override(None).unwrap();
        assert_eq!(
            config.master_address.as_ref().unwrap().to_string(),
            "10.0.0.10:15600"
        );

        config
            .apply_master_address_override(Some("master:15600"))
            .unwrap();
        assert_eq!(
            config.master_address.as_ref().unwrap().to_string(),
            "master:15600"
        );

        let err = config
            .apply_master_address_override(Some("master"))
            .unwrap_err();
        assert!(err.contains(MASTER_ADDRESS_ENV), "{err}");
    }

    #[test]
    fn test_role_checks() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;

        config.role = NodeRole::Master;
        assert!(config.is_master());
        assert!(!config.is_replica());

        config.role = NodeRole::Replica;
        assert!(!config.is_master());
        assert!(config.is_replica());
    }
}

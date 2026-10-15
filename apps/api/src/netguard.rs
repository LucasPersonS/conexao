//! Ping de servidores informados pelos usuários sem abrir caminho para a rede interna (SSRF).
//!
//! O endereço é resolvido UMA vez (SRV incluído), todos os IPs encontrados são
//! conferidos e a conexão é feita no IP já validado. Assim um DNS que muda de
//! resposta entre a checagem e a conexão (rebinding) ou um SRV apontando para
//! `postgres:5432` não chegam a máquinas internas.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use async_minecraft_ping::ConnectionConfig;

use crate::status::split_address;

const DNS_TIMEOUT: Duration = Duration::from_secs(5);

/// Destino já resolvido e aprovado.
#[derive(Debug, Clone)]
pub struct Target {
    /// Nome mandado no handshake (como o cliente do Minecraft faz).
    pub host: String,
    pub port: u16,
    pub addr: SocketAddr,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// O nome não existe ou o DNS não respondeu.
    NotFound,
    /// Aponta para rede local, loopback ou outro endereço reservado.
    Private,
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (b & 0xc0) == 64) // 100.64.0.0/10 (CGNAT)
        || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24
        || (a == 198 && (b & 0xfe) == 18) // 198.18.0.0/15
        || a >= 240)
}

fn public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return public_v4(v4);
    }
    let s = ip.segments();
    // 64:ff9b::/96 (NAT64) leva a um IPv4: confere o IPv4 de dentro.
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        let [a, b] = s[6].to_be_bytes();
        let [c, d] = s[7].to_be_bytes();
        return public_v4(Ipv4Addr::new(a, b, c, d));
    }
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 (rede local única)
        || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 (link local)
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentação
        || s[..6] == [0, 0, 0, 0, 0, 0]) // ::/96 (IPv4 compatível, obsoleto)
}

/// Endereço roteável na internet pública.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => public_v6(v6),
    }
}

/// Aplica a regra a uma lista de IPs resolvidos: todos precisam ser públicos.
fn check(addrs: &[SocketAddr], allow_private: bool) -> Result<SocketAddr, ResolveError> {
    let first = *addrs.first().ok_or(ResolveError::NotFound)?;
    if !allow_private && addrs.iter().any(|a| !is_public(a.ip())) {
        return Err(ResolveError::Private);
    }
    Ok(first)
}

/// Resolve `host[:porta]` (sem porta, consulta o SRV `_minecraft._tcp`) e confere os IPs.
pub async fn resolve(address: &str, allow_private: bool) -> Result<Target, ResolveError> {
    let (host, port) = split_address(address);
    let (host, port) = match port {
        Some(port) => (host, port),
        None => async_minecraft_ping::lookup_srv_record(&host, DNS_TIMEOUT).await.unwrap_or((host, 25565)),
    };
    let addrs: Vec<SocketAddr> = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host.as_str(), port)))
        .await
        .map_err(|_| ResolveError::NotFound)?
        .map_err(|_| ResolveError::NotFound)?
        .collect();
    let addr = check(&addrs, allow_private)?;
    Ok(Target { host, port, addr })
}

/// Configuração de ping que conecta no IP aprovado.
pub fn ping_config(target: &Target, timeout: Duration) -> ConnectionConfig {
    ConnectionConfig::build(target.host.clone())
        .with_port(target.port)
        .with_socket_addr(target.addr)
        .with_timeout(timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn blocks_internal_addresses() {
        for blocked in [
            "10.0.0.5",
            "172.18.0.2",
            "192.168.1.10",
            "127.0.0.1",
            "0.0.0.0",
            "169.254.169.254",
            "100.64.1.1",
            "198.18.0.1",
            "255.255.255.255",
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:10.0.0.1",
            "::ffff:127.0.0.1",
            "64:ff9b::a00:1",
        ] {
            assert!(!is_public(ip(blocked)), "{blocked} deveria ser bloqueado");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for allowed in ["1.1.1.1", "8.8.8.8", "100.128.0.1", "2606:4700:4700::1111", "::ffff:8.8.8.8"] {
            assert!(is_public(ip(allowed)), "{allowed} deveria ser liberado");
        }
    }

    #[test]
    fn every_resolved_address_must_be_public() {
        let public: SocketAddr = "8.8.8.8:25565".parse().unwrap();
        let private: SocketAddr = "10.0.0.1:25565".parse().unwrap();
        assert_eq!(check(&[public], false), Ok(public));
        assert_eq!(check(&[public, private], false), Err(ResolveError::Private));
        assert_eq!(check(&[public, private], true), Ok(public));
        assert_eq!(check(&[], false), Err(ResolveError::NotFound));
    }
}

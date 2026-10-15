//! Cliente RCON mínimo (protocolo Source RCON usado pelo Minecraft).

use std::time::Duration;

use eyre::{WrapErr, bail, eyre};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const AUTH: i32 = 3;
const COMMAND: i32 = 2;
const TIMEOUT: Duration = Duration::from_secs(5);

pub struct Rcon {
    stream: TcpStream,
    next_id: i32,
}

pub fn encode(id: i32, kind: i32, body: &str) -> Vec<u8> {
    let length = i32::try_from(body.len() + 10).unwrap_or(i32::MAX);
    let mut packet = Vec::with_capacity(body.len() + 14);
    packet.extend_from_slice(&length.to_le_bytes());
    packet.extend_from_slice(&id.to_le_bytes());
    packet.extend_from_slice(&kind.to_le_bytes());
    packet.extend_from_slice(body.as_bytes());
    packet.extend_from_slice(&[0, 0]);
    packet
}

impl Rcon {
    pub async fn connect(address: &str, password: &str) -> eyre::Result<Self> {
        let stream = tokio::time::timeout(TIMEOUT, TcpStream::connect(address))
            .await
            .map_err(|_| eyre!("tempo esgotado conectando ao RCON em {address}"))?
            .wrap_err_with(|| format!("não foi possível conectar ao RCON em {address}"))?;
        let mut rcon = Self { stream, next_id: 1 };
        let id = rcon.send(AUTH, password).await?;
        // O servidor responde com id -1 quando a senha está errada.
        loop {
            let (response_id, kind, _) = rcon.read().await?;
            if response_id == -1 {
                bail!("senha RCON incorreta");
            }
            if response_id == id && kind == 2 {
                break;
            }
        }
        Ok(rcon)
    }

    async fn send(&mut self, kind: i32, body: &str) -> eyre::Result<i32> {
        if body.len() > 1446 {
            bail!("comando RCON longo demais");
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.stream.write_all(&encode(id, kind, body)).await?;
        Ok(id)
    }

    async fn read(&mut self) -> eyre::Result<(i32, i32, String)> {
        tokio::time::timeout(TIMEOUT, async {
            let length = self.stream.read_i32_le().await?;
            if !(10..=4110).contains(&length) {
                bail!("pacote RCON inválido ({length} bytes)");
            }
            let id = self.stream.read_i32_le().await?;
            let kind = self.stream.read_i32_le().await?;
            let mut body = vec![0u8; usize::try_from(length - 8).unwrap_or(0)];
            self.stream.read_exact(&mut body).await?;
            body.truncate(body.len().saturating_sub(2));
            Ok((id, kind, String::from_utf8_lossy(&body).to_string()))
        })
        .await
        .map_err(|_| eyre!("tempo esgotado aguardando resposta RCON"))?
    }

    pub async fn command(&mut self, command: &str) -> eyre::Result<String> {
        let id = self.send(COMMAND, command).await?;
        loop {
            let (response_id, _, body) = self.read().await?;
            if response_id == id {
                return Ok(body);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn packet_layout() {
        let packet = super::encode(7, 2, "list");
        assert_eq!(&packet[0..4], &14i32.to_le_bytes());
        assert_eq!(&packet[4..8], &7i32.to_le_bytes());
        assert_eq!(&packet[8..12], &2i32.to_le_bytes());
        assert_eq!(&packet[12..16], b"list");
        assert_eq!(&packet[16..], &[0, 0]);
    }
}

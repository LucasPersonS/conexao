# Contas não originais (login pelo Discord)

Jogadores sem Minecraft original entram no launcher com o **Discord**, escolhem um nick e
jogam nos servidores que **aceitam contas não originais**. Servidores só para originais
continuam como estão.

## Como funciona

1. **Conta**: o launcher abre o login do Discord. No primeiro acesso o jogador escolhe o nick
   (3–16 caracteres, letras, números e `_`). O nick é único no Conexão e **não pode ser o nick
   de uma conta original** (a API confere na Mojang). O UUID é o UUID offline do nick
   (`OfflinePlayer:<nick>`), o mesmo que o servidor calcula em `online-mode=false`.
2. **Jogar**: o launcher abre o Minecraft sem token da Microsoft (`user_type=legacy`).
3. **Passe de entrada**: em `online-mode=false` o servidor não sabe quem é quem. Ao clicar em
   Jogar, o launcher pede um passe à API por `join.<domínio>`. A API registra *nick + IP* por
   15 minutos e repassa ao Companion pelo WebSocket. No login, o Companion confere o passe
   na hora (sem chamada de rede) e expulsa quem não tem. Depois de sair, o jogador pode
   reconectar pelo próprio jogo por 10 minutos.
4. **Whitelist**: nesses servidores a whitelist sai com o UUID offline de todos, inclusive
   dos jogadores originais.

## Configurar (uma vez, na VPS)

1. **App do Discord**: em discord.com/developers → *New Application* → nome "Conexão Launcher".
   Em *OAuth2* → *Redirects*, adicione `https://api.<domínio>/v1/auth/discord/callback`.
   Copie o *Client ID* e gere o *Client Secret*.
2. **`.env` da VPS**:
   ```
   CONEXAO_DISCORD_CLIENT_ID=...
   CONEXAO_DISCORD_CLIENT_SECRET=...
   CONEXAO_JOIN_DOMAIN=join.conexaosmps.com.br
   ```
   O `atualizar.sh` adiciona `CONEXAO_JOIN_DOMAIN` sozinho se faltar.
3. **DNS**: registro **A** `join` apontando para a VPS. **Sem AAAA** e, na Cloudflare,
   **"Somente DNS"** (nuvem cinza). O Java conecta no servidor por IPv4; se o passe saísse
   por IPv6 ou pelo proxy da Cloudflare, o IP não bateria.
4. `bash /opt/conexao/deploy/atualizar.sh`.

## Ligar num servidor (dono do servidor)

1. Instalar o **Conexão Companion 0.2+** e vincular.
2. No `server.properties`: `online-mode=false` e `enforce-secure-profile=false`. Reiniciar.
3. Painel → servidor → **Aceitar contas não originais** → Salvar. A API só deixa ligar com o
   Companion 0.2+ conectado e as duas opções acima certas.

**Atenção:** trocar um servidor existente de online para offline muda o UUID de todos os
jogadores. Inventários e dados salvos pelo UUID antigo não aparecem. Faça a troca antes de
abrir o servidor ou migre os arquivos de `world/playerdata`.

## Limites conhecidos

- Jogadores atrás do mesmo IP (CGNAT de operadora, mesma casa) dividem o IP: alguém nessa
  mesma rede que saiba o nick poderia entrar nos 15 minutos do passe.
- Proxies na frente do servidor (Velocity, BungeeCord, TCPShield) escondem o IP real do
  jogador: o Companion passa a ver o IP do proxy. Por enquanto não é suportado.
- Um impostor sem passe é barrado no login, mas o Minecraft já desconecta o jogador
  verdadeiro com o mesmo nick antes disso (comportamento do `online-mode=false`).
- Contas não originais não têm skin (aparece Steve/Alex).
- O EULA da Mojang exige que o jogador tenha o jogo. A opção é do dono de cada servidor.

# Lista de espera (acesso antecipado)

Com `CONEXAO_WAITLIST=true` (padrão no `docker-compose.yml`), só usa o launcher quem a equipe liberou.

## Como funciona

1. **Site** (`conexaosmps.com.br`): a home virou a lista. A pessoa clica em *Entrar na lista com o
   Discord*, volta para o site, escolhe o tipo de conta (original ou não original) e informa o nick.
   O nick de conta original é conferido na Mojang. A posição na fila aparece na hora, e o site chama
   para o Discord da comunidade (`discord.gg/conexaosmps`) em vários pontos.
2. **Painel** → *Equipe Conexão* → **Lista de espera**: aprovar, recusar, devolver para a fila,
   *Liberar os próximos N* (uma leva por ordem de chegada) e *Liberar pelo nick* (para quem não passou
   pelo site: conta que já existe no Conexão ou conta original).
3. **Liberado**: o botão *Baixar para Windows* aparece no site para essa pessoa, e o launcher aceita a
   conta (a original pelo UUID da Mojang, a não original pelo Discord).

### O que a API bloqueia para quem não foi liberado

- Login no launcher (`/v1/auth/minecraft`, `/v1/auth/discord`, cadastro e renovação da conta não original).
- Manifesto dos modpacks (`/v1/servers/<slug>/manifest`): sem ele não instala nada. O launcher 0.1.6+
  manda o token nessa chamada; versões antigas recebem o aviso e o card de atualização.
- `/baixar`: quem não foi liberado volta para `conexaosmps.com.br/#lista`.

A mensagem do launcher manda para o site e para o Discord.

> Os arquivos em `/launcher/*` continuam públicos (o atualizador automático precisa deles). Quem
> conseguir o `.exe` por fora instala, mas não entra: o bloqueio real é o login e o manifesto.

### Quem passa direto (sem fila)

- Equipe Conexão (`is_staff`, `CONEXAO_STAFF_UUIDS`) e, no site, os IDs em `CONEXAO_STAFF_DISCORD_IDS`.
- Quem já tinha conta antes da lista (a migração `0010_waitlist.sql` libera todas as contas existentes).
- Quem está na equipe (dono, admin ou moderador) de qualquer servidor parceiro.

Recusar (ou *Tirar acesso*) remove o acesso da conta ligada àquele pedido no próximo login.

## Configurar (uma vez)

1. No [Discord Developer Portal](https://discord.com/developers/applications) → o app do Conexão →
   **OAuth2** → **Redirects**, adicione (além do que já existe para o launcher):

   ```
   https://conexaosmps.com.br/v1/waitlist/discord/callback
   ```

2. No `deploy/.env` da VPS (opcional; os padrões já servem):

   ```
   CONEXAO_WAITLIST=true
   # IDs do Discord da equipe (Discord → Configurações → Avançado → Modo desenvolvedor → clique direito → Copiar ID)
   CONEXAO_STAFF_DISCORD_IDS=123456789012345678
   CONEXAO_DISCORD_INVITE=https://discord.gg/conexaosmps
   ```

   O login do site usa o mesmo `CONEXAO_DISCORD_CLIENT_ID` / `CONEXAO_DISCORD_CLIENT_SECRET` das contas
   não originais. Sem eles, o botão do site avisa que o login está indisponível.

3. Lance o launcher 0.1.6 **antes** (ou junto) de atualizar a VPS, como em `COMANDOS-ATUALIZAR.txt`:
   tag → esperar o GitHub → `bash /opt/conexao/deploy/atualizar.sh`.

Para desligar a lista: `CONEXAO_WAITLIST=false` e reiniciar a API. O site volta a mostrar o download
para todo mundo.

## Rotas

| Rota | Quem | O quê |
| --- | --- | --- |
| `GET /v1/waitlist/discord` | site | vai para a autorização do Discord (`identify`) |
| `GET /v1/waitlist/discord/callback` | Discord | grava a sessão do site (cookie `cx_lista`, 30 dias) |
| `GET /v1/waitlist/me` | site | tamanho da fila e situação de quem está logado |
| `POST /v1/waitlist/me` | site | entra na fila / troca o nick (mantém a posição) |
| `POST /v1/waitlist/sair` | site | sai |
| `GET /v1/waitlist/comunidade` | site | membros e online do Discord (cache de 10 min) |
| `GET /v1/staff/waitlist?status=` | equipe | lista (`pending`, `approved`, `rejected`) |
| `POST /v1/staff/waitlist/{id}` | equipe | `{ status, note? }` |
| `POST /v1/staff/waitlist/next` | equipe | `{ count }`: libera os N primeiros |
| `POST /v1/staff/waitlist/manual` | equipe | `{ nick }`: libera pelo nick |

O Caddy repassa `/v1/waitlist/*` e `/baixar` no domínio do site para a API, para o cookie ficar só
naquele domínio.

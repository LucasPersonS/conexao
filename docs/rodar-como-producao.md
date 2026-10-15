# Rodar o Conexão no PC como se fosse o servidor real

Este guia sobe no seu Windows **a mesma stack da VPS**: PostgreSQL, API e Caddy em
contêineres (Podman), com o mesmo `docker-compose.yml`, o mesmo `.env` e o mesmo HTTPS. Depois disso,
você liga o painel, o launcher, o agente de um servidor Minecraft de teste e o Mercado Pago.

| Peça | Endereço local | Na VPS seria |
|---|---|---|
| API (pelo Caddy, HTTPS) | https://api.localhost | https://api.conexao.gg |
| API (direto, para o launcher) | http://localhost:8080 | — |
| Painel | https://painel.localhost | https://painel.conexao.gg |
| Banco | dentro do Podman (volume `pgdata`) | igual |
| Arquivos dos modpacks | volume `files` | igual |

> Para programar no dia a dia (recompilar rápido) continue usando o
> `podman compose -f deploy/docker-compose.dev.yml up -d` + `cargo run`. Este guia é para ensaiar o deploy.

---

## 0. Pré-requisitos (uma vez)

- **Podman Desktop** (ou `winget install RedHat.Podman`) com WSL 2 ligado.
- **docker-compose** instalado: o `podman compose` usa ele por baixo. A mensagem
  "Executing external compose provider" que aparece em todo comando é normal.
- Máquina do Podman em modo **rootful**, para o Caddy poder abrir as portas 80 e 443:

  ```powershell
  podman machine init            # só na primeira vez
  podman machine stop
  podman machine set --rootful
  podman machine start
  podman version                 # deve mostrar Client e Server
  ```

- Desligue o limite de processos (a máquina do Podman no WSL não tem o controlador
  `pids`; sem isto todo contêiner falha com *controller `pids` is not available*):

  ```powershell
  podman machine ssh "sudo mkdir -p /etc/containers/containers.conf.d && printf '[containers]\npids_limit = 0\n' | sudo tee /etc/containers/containers.conf.d/pids.conf"
  podman machine stop; podman machine start
  ```

> Os arquivos continuam com o nome `docker-compose*.yml`, o formato padrão que o
> Podman lê. O `.dockerignore` na raiz evita enviar a pasta `target` (16 GB) para o build.
- **Rust**, **Node.js 22**, **JDK 21** e os pré-requisitos do Tauri (veja o README).
- Opcional: **cloudflared**, só para testar o webhook do Mercado Pago (passo 9):
  `winget install Cloudflare.cloudflared`.

Todos os comandos abaixo são no **PowerShell**, a partir de `Documents\Conexão`.

---

## 1. Gerar os segredos

```powershell
function Novo-Segredo($bytes) { -join ((1..$bytes) | ForEach-Object { '{0:x2}' -f (Get-Random -Maximum 256) }) }
Novo-Segredo 32   # → CONEXAO_JWT_SECRET
Novo-Segredo 24   # → CONEXAO_STAFF_TOKEN
Novo-Segredo 16   # → POSTGRES_PASSWORD
```

Chave que assina os modpacks (gera um par: a secreta vai no `.env`, a pública no launcher):

```powershell
cargo run -p conexao-api -- gerar-chave-assinatura
```

(Roda direto no Windows, sem contêiner. Não precisa de banco nem de `.env`.)

Guarde as duas linhas que aparecem (`CONEXAO_SIGNING_KEY=…` e `CONEXAO_MANIFEST_PUBKEY=…`).


---

## 2. Preencher o `deploy\.env`

O arquivo já existe (cópia do `.env.example`, fora do Git). Deixe assim:

```ini
CONEXAO_DOMAIN=api.localhost
CONEXAO_PANEL_DOMAIN=painel.localhost
POSTGRES_PASSWORD=<segredo de 16>
CONEXAO_JWT_SECRET=<segredo de 32>
CONEXAO_STAFF_TOKEN=<segredo de 24>
CONEXAO_CORS_ORIGINS=https://painel.localhost
CONEXAO_SIGNING_KEY=<do passo 1>
CONEXAO_STAFF_UUIDS=<seu UUID do Minecraft>
CONEXAO_MP_ACCESS_TOKEN=<token APP_USR-… da conta de teste VENDEDORA>
CONEXAO_MP_WEBHOOK_SECRET=
```

- **Seu UUID**: https://mcuuid.net com o seu nick. Isso faz de você equipe Conexão
  (aprovar servidores, moderar perfis).
- Os demais campos podem ficar como no exemplo.
- **Nunca** coloque o `.env` no Git nem mande os valores por chat.

---

## 3. Gerar o painel (build de produção)

O Caddy serve o painel já compilado, como na VPS:

```powershell
cd apps\panel
npm ci
$env:VITE_CONEXAO_API_URL = "https://api.localhost"
npm run build
Remove-Item Env:VITE_CONEXAO_API_URL
cd ..\..
```

---

## 4. Subir a stack

```powershell
cd deploy
podman compose -f docker-compose.yml -f docker-compose.local.yml up -d --build
podman compose ps                    # postgres (healthy), api e caddy "running"
podman logs -f deploy-api-1           # Ctrl+C para sair dos logs
```

A API aplica as migrações do banco sozinha ao iniciar. Teste:

```powershell
curl.exe http://localhost:8080/v1/featured/plans
```

Deve responder com os planos e `"enabled":true` (se o token do Mercado Pago estiver no `.env`).

---

## 5. Confiar no HTTPS local

O Caddy cria uma autoridade certificadora local para `*.localhost`. Importe-a no Windows
para o navegador não mostrar aviso:

```powershell
podman cp deploy-caddy-1:/data/caddy/pki/authorities/local/root.crt .\caddy-root.crt
Import-Certificate -FilePath .\caddy-root.crt -CertStoreLocation Cert:\CurrentUser\Root
Remove-Item .\caddy-root.crt
```

Feche e abra o navegador e acesse **https://painel.localhost**.

> O launcher não usa esse certificado (ele só confia em certificados públicos). Por isso
> ele fala com a API direto em `http://localhost:8080`.

---

## 6. Rodar o launcher apontando para a stack

```powershell
cd apps\launcher
npm ci
$env:CONEXAO_API_URL = "http://localhost:8080"
$env:CONEXAO_MANIFEST_PUBKEY = "<do passo 1>"
$env:CONEXAO_DISCORD_APP_ID = "<ID do app no Discord, opcional>"
npx tauri dev
```

Para testar o **instalador** como o jogador vai receber: `npx tauri build`
(sai em `target\release\bundle\nsis\`).

> Essas variáveis entram no launcher na hora de compilar. Se mudar alguma, rode de novo.

---

## 7. Fluxo completo de ponta a ponta

1. **Launcher**: entre com sua conta Microsoft.
2. **Painel** (https://painel.localhost): aparece um código; confirme no launcher em
   **Menu do avatar → Conectar painel web**.
3. **Cadastrar servidor**: use o endereço `host.containers.internal:25565` para um servidor
   Minecraft rodando no seu PC. A API está dentro de um contêiner, então `localhost` lá
   dentro é o próprio contêiner.
4. **Aprovar**: como você está em `CONEXAO_STAFF_UUIDS`, aprove o servidor na página
   **Equipe Conexão**.
5. **Publicar modpack**: aba **Modpack** → escolher a pasta → conferir → publicar.
6. **Jogar**: no launcher, abra o servidor e clique em **Jogar**. Ele baixa os arquivos,
   confere a assinatura e abre o jogo.

---

## 8. Agente no servidor Minecraft de teste

No `server.properties` do servidor de teste: `white-list=true`, `enable-rcon=true`,
`rcon.port=25575`, `rcon.password=<uma senha>`.

No painel, aba **Agente** → **Gerar token**. Depois crie `conexao-agent.toml` na pasta do
servidor:

```toml
api_url = "ws://localhost:8080/v1/agent/connect"   # na VPS: wss://api.conexao.gg/...
token = "cagt_..."
server_dir = "C:/caminho/do/servidor"

[rcon]
address = "127.0.0.1:25575"
password = "<a mesma do server.properties>"
```

```powershell
cargo run -p conexao-agent -- --config "C:\caminho\do\servidor\conexao-agent.toml"
```

A aba **Agente** do painel deve mostrar **Conectado**. Peça whitelist pelo launcher e libere
no painel: o nick entra no `whitelist.json` na hora.

---

## 9. Mercado Pago (destaque pago)

### Pagamento de teste (sem webhook)

1. No painel, abra o servidor aprovado → **Destaque na home** → **Destacar**.
2. No Mercado Pago, entre com a **conta de teste compradora**. A senha fica em
   Mercado Pago Developers → Suas integrações → Contas de teste.
3. Pague com cartão de teste: **5031 4332 1540 6351**, validade 11/30, CVV 123,
   CPF 12345678909, titular **APRO** (aprovado) ou **OTHE** (recusado).
4. Ao voltar para o painel, ele consulta o pagamento e liga o destaque.

### Testar o webhook (aviso automático)

O Mercado Pago precisa de um endereço HTTPS público. Abra um túnel temporário:

```powershell
cloudflared tunnel --url http://localhost:8080
```

Copie o endereço `https://….trycloudflare.com` e:

1. No `deploy\.env`, adicione `CONEXAO_PUBLIC_URL=https://….trycloudflare.com`.
2. Recrie a API: `podman compose -f docker-compose.yml -f docker-compose.local.yml up -d api`.
3. No Mercado Pago Developers → app de teste → **Webhooks**, cadastre
   `https://….trycloudflare.com/v1/payments/mercadopago/webhook` com o evento **Pagamentos**.
4. Copie a **assinatura secreta** para `CONEXAO_MP_WEBHOOK_SECRET` no `.env` e recrie a API de novo.
5. Faça outra compra de teste e veja nos logs: `podman logs -f deploy-api-1`.

Ao terminar, apague a linha `CONEXAO_PUBLIC_URL` e recrie a API. O endereço do túnel muda
toda vez que você abre o túnel.

---

## 10. Manutenção

| Quero… | Comando (na pasta `deploy`) |
|---|---|
| Ver logs | `podman logs -f deploy-api-1` |
| Atualizar depois de mudar o código | `podman compose -f docker-compose.yml -f docker-compose.local.yml up -d --build` |
| Atualizar o painel | repetir o passo 3 (o Caddy pega sozinho) |
| Parar tudo | `podman compose down` |
| Backup do banco | `podman exec deploy-postgres-1 pg_dump -U conexao conexao > backup.sql` |
| Restaurar backup | `Get-Content backup.sql \| podman exec -i deploy-postgres-1 psql -U conexao conexao` |

Se o banco foi criado quando o projeto ainda se chamava CENTRAL, troque `conexao conexao` por `central central` nos dois comandos.
| Começar do zero (apaga banco e arquivos!) | `podman compose down -v` |

---

## 11. Diferenças para a VPS

Na VPS real, só muda isto:

- No `.env`: `CONEXAO_DOMAIN` e `CONEXAO_PANEL_DOMAIN` com os domínios reais (DNS apontando
  para a VPS), `CONEXAO_CORS_ORIGINS=https://painel.<domínio>` e o access token de
  **produção** do app *centrallauncher*.
- Sobe **sem** o `docker-compose.local.yml`: a porta 8080 fica fechada e só 80/443 abrem.
  O Caddy emite certificados públicos (Let's Encrypt) sozinho.
- O launcher é compilado com `CONEXAO_API_URL=https://api.<domínio>`. No GitHub, isso é a
  variável do CI.
- Agente: `api_url = "wss://api.<domínio>/v1/agent/connect"`.
- Webhook do Mercado Pago cadastrado no app de produção com o domínio real.

---

## Problemas comuns

| Sintoma | Causa provável |
|---|---|
| `api` reinicia sem parar | Falta `CONEXAO_JWT_SECRET` (mínimo 32 caracteres) ou o banco não subiu. Veja `podman logs deploy-api-1`. |
| Painel diz "erro de rede" | `CONEXAO_CORS_ORIGINS` diferente de `https://painel.localhost`, ou o painel foi compilado sem `VITE_CONEXAO_API_URL`. |
| Navegador avisa "conexão não segura" | Faltou o passo 5, ou o navegador ficou aberto durante a importação. |
| Launcher: "assinatura inválida" | `CONEXAO_MANIFEST_PUBKEY` do launcher não é o par do `CONEXAO_SIGNING_KEY` do `.env`. |
| Servidor aparece offline | Use `host.containers.internal:25565`, e não `localhost`. |
| "Pagamentos ainda não estão ativos" | `CONEXAO_MP_ACCESS_TOKEN` vazio. Preencha e recrie a API. |
| Mercado Pago: "não é possível pagar a si mesmo" | Você entrou com a conta vendedora; use a compradora. |
| Caddy: "permission denied" nas portas 80/443 | Máquina do Podman em modo rootless. Rode `podman machine set --rootful` (veja o passo 0). |
| "Cannot connect to the Docker daemon" | Máquina do Podman parada: `podman machine start`. |
| Build parado em "Sending build context" | Falta o `.dockerignore` na raiz do projeto. |
| *controller `pids` is not available* | Falta o ajuste de `pids_limit` do passo 0. |
| *unable to upgrade to tcp, received 500* | Incompatibilidade do `podman compose run/exec/logs`. Use `podman run/exec/logs` direto (nomes em `podman ps`). |

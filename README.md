# Conexão

Launcher de Minecraft para a comunidade de **Minecraft Roleplay**, no estilo FiveM:
o jogador entra com a conta Microsoft, vê a lista de servidores parceiros e clica em
**Jogar**. O Conexão instala Java, loader (NeoForge, Forge ou Fabric) e o modpack do
servidor e abre o jogo já conectando no IP.

Os administradores publicam versões do modpack, gerenciam a whitelist e o agente
do Conexão aplica a whitelist no servidor em tempo real.

> Suporta Minecraft **1.18.2 ou mais novo**. Licença **GPL-3.0** (reutiliza o
> `theseus`, app-lib do [Modrinth App](https://github.com/modrinth/code)).

## Estrutura

```
apps/
  launcher/         App desktop: Tauri 2 (Rust) + Vue 3
  installer/        Instalador visual (Tauri 2 + Vue 3): baixa, confere e instala o launcher
  panel/            Painel web dos administradores (Vue 3)
  site/             Site público com a página de download (HTML estático, sem build)
  api/              API central (axum + PostgreSQL)
  agent/            Agente que roda ao lado do servidor Minecraft (whitelist + RCON)
  publish/          conexao-publish: publica uma versão do modpack pela linha de comando
crates/
  conexao-types/    Tipos compartilhados (manifesto, servidores, whitelist, protocolo do agente)
  conexao-pack/     Publicador: análise da pasta do modpack e diff entre versões
packages/
  profile-ui/       Card de perfil compartilhado (launcher, painel e página perfil.<domínio>/u/<nick>)
vendor/             Código do Modrinth reutilizado (theseus, daedalus e dependências)
branding/           Logo, ícones, letreiro e banner do Conexão
deploy/             docker-compose para a VPS (API + PostgreSQL + Caddy: site, API e painel)
```

## Como as peças conversam

1. O admin roda `conexao-publish` na pasta do modpack. Ele analisa `mods/`, `config/`,
   `defaultconfigs/` e `resourcepacks/`, mostra o diff, envia só os arquivos novos
   (endereçados por SHA-1) e registra a versão na API.
2. A API guarda o **manifesto** da versão e aponta o canal `stable` (ou `beta`) para ela.
   Rollback é só apontar o canal para uma versão anterior.
3. No launcher, **Jogar** confere a whitelist, baixa o manifesto (com `ETag`), converte
   em `.mrpack` e entrega ao `theseus`, que instala Java, loader e arquivos e abre o
   jogo com Quick Play no IP do servidor.
4. Quando um moderador aprova ou bane alguém, a API avisa o agente pelo WebSocket; o
   agente regrava o `whitelist.json` e executa `whitelist reload` via RCON.

Configs publicadas são `initial` por padrão (só instaladas se o jogador ainda não tem
o arquivo). Use `--managed config/arquivo.toml` para forçar sobrescrita.

## Identidade visual

Tema escuro com as cores do logo do Conexão: verde-lima `#b7fe00` (ações, destaques e estados ativos),
preto `#0a0f11` (fundo) e branco `#f2f6ef` (texto). Títulos em **Fredoka** (arredondada e pesada, como o
letreiro do logo); rótulos pequenos em **Silkscreen**, lembrando os blocos de pixel. A grade de fundo, as
cantoneiras e os blocos de pixel vêm dos banners. Os tokens ficam em `apps/launcher/src/style.css` e
`apps/panel/src/style.css` (mesmos nomes nos dois).

Arquivos em `branding/`:

| Arquivo | Uso |
| --- | --- |
| `logo.png` | Ícone do app (1024 px, cantos arredondados); os ícones do instalador saem dele (`apps/launcher/src-tauri/icons`, `npx tauri icon branding/logo.png`) |
| `icon-rounded-512.png` | Ícone em 512 px |
| `icon-quadrado-1024.png` | Ícone quadrado, para Discord (Rich Presence) e redes |
| `marca.png` | Só o "C" do logo, com fundo transparente |
| `wordmark.png` | Letreiro "Conexão" com fundo transparente |
| `banner.jpg` | Banner "Junte-se agora" (tela de login do launcher e do painel) |

## Troca de nome: CENTRAL → Conexão

O projeto se chamava CENTRAL. O que mudou e o que fazer fora do código:

- **Variáveis de ambiente**: todas passaram de `CENTRAL_*` para `CONEXAO_*` (o `.env` da VPS e o
  `apps/api/.env` já foram convertidos, com os mesmos valores). No GitHub, renomeie as variáveis do
  repositório `CENTRAL_API_URL`, `CENTRAL_MANIFEST_PUBKEY` e `CENTRAL_DISCORD_APP_ID` para `CONEXAO_*`.
  O painel usa `VITE_CONEXAO_API_URL`.
- **Banco**: o `deploy/.env` ganhou `POSTGRES_DB=central` e `POSTGRES_USER=central` para manter o banco
  já criado. Numa VPS nova, apague as duas linhas e o banco nasce como `conexao`. As migrações não foram
  alteradas (o `sqlx` confere o conteúdo delas).
- **Launcher**: identificador `gg.conexao.launcher`, link de convite `conexao://servidor/<slug>`, instalador
  `Conexao_<versão>_x64-setup.exe` (nome do pacote sem acento para não quebrar URLs e o atualizador; a janela
  e toda a interface mostram "Conexão"). Na primeira abertura o launcher move sozinho os dados da pasta
  antiga (`gg.central.launcher`): conta, instâncias, preferências e favoritos. Quem tinha o CENTRAL instalado
  recebe o Conexão como um app novo; o CENTRAL antigo pode ser desinstalado.
- **Cabeçalhos HTTP**: `X-Conexao-Signature` e `X-Conexao-Staff-Token` (API e launcher precisam ser
  atualizados juntos).
- **Companion**: mod `conexao_companion`, arquivo `conexao-companion-neoforge-1.21.1.jar` (refaça o build e
  publique com esse nome). Servidores já vinculados continuam: o mod lê o `config/central-companion.json` antigo.
- **Discord**: a imagem do Rich Presence agora se chama `conexao`.
- **Painel**: a área da equipe virou `/equipe-conexao` (o endereço antigo redireciona).

## Painel web

O painel (`apps/panel`) é onde dono, admins e moderadores cuidam do servidor:

- **Visão geral**: nome, descrição, endereço, tags, ícone, banner, Discord, whitelist obrigatória.
- **Modpack**: publicar uma versão nova direto do navegador (selecionar a pasta → diff →
  publicar), canal estável/beta e rollback com um clique.
- **Whitelist**: pedidos, liberar por nick, recusar e banir (o agente aplica na hora).
- **Equipe**: admins e moderadores.
- **Agente**: status da conexão e geração do token com o `conexao-agent.toml` pronto.
- **Histórico**: quem fez o quê.
- **Equipe Conexão** (staff): aprovar servidores e ligar o destaque pago por N dias.

Login: o painel mostra um código e o admin confirma no launcher (**Conectar painel web**).
Assim o painel não precisa de app Microsoft próprio. Para marcar alguém como equipe
Conexão, coloque o UUID Minecraft dele em `CONEXAO_STAFF_UUIDS`.

```powershell
cd apps\panel
npm install
$env:VITE_CONEXAO_API_URL = "http://localhost:8080"
npm run dev          # http://localhost:5174
```

## Convites e imagens

- Cada servidor aprovado tem um link de convite: `https://<api>/convite/<slug>`. No Discord vira um card
  com banner e nome; o botão **Abrir no Conexão** usa `conexao://servidor/<slug>` e o launcher abre direto
  na página do servidor (se estiver fechado, abre; se estiver aberto, foca a janela).
- `https://<api>/baixar` leva ao instalador mais recente (do `latest.json`) ou a `CONEXAO_DOWNLOAD_URL`.
- Ícone e banner são enviados pelo painel (PNG/JPEG/WebP, até 1 MB / 5 MB) e servidos em `/images/`.
  No cadastro, o ícone do próprio servidor (`server-icon.png`, lido pelo ping) vira o ícone no Conexão.

## Perfis dos jogadores

- O jogador edita o perfil no launcher (menu da conta → **Meu perfil**): nome de exibição, pronomes, bio,
  redes (Discord, YouTube, Twitch, TikTok, Instagram, X, Bluesky), fundo em gradiente ou imagem, cartão
  claro/escuro, cor de destaque, moldura animada, fonte e efeito no nome. A pré-visualização é ao vivo.
- O tema é salvo como dados (cores e IDs do catálogo em `packages/profile-ui/src/catalog.json`), nunca como
  CSS livre. O mesmo `profile.css` desenha o card no launcher, no painel e na página pública `/u/<nick>`
  (com prévia para Discord).
- Os moderadores veem o card em cada pedido de whitelist. O Discord pode ficar visível para todos, só para a
  equipe dos servidores ou para ninguém.
- Moderação: filtro de palavras (mais `CONEXAO_BLOCKED_WORDS_FILE`), links só nos campos próprios e com
  domínios conferidos, fundos enviados passam pela fila da equipe e qualquer jogador pode denunciar um perfil.
  Tudo isso fica em **Equipe Conexão → Moderação de perfis**.
- Itens premium estão marcados no catálogo, mas só são exigidos com `CONEXAO_PREMIUM_ENFORCED=true`.

## Eventos, notícias e avisos

- A equipe do servidor (moderador ou acima) publica eventos e notícias no painel, aba **Eventos e notícias**.
  No launcher aparecem na página do servidor; os eventos dos próximos 14 dias também entram em
  **Próximos eventos** na home (favoritos e servidores jogados primeiro).
- O jogador pode marcar **Lembrar**: o launcher avisa com uma notificação do sistema 10 minutos antes.
- Servidor lotado: **Avise-me quando abrir vaga** pinga só aquele servidor a cada 20 s (por até 3 h) e notifica.

## Desempenho e Discord

- **Memória automática** (padrão): calculada pelo número de mods da instância e pela RAM do computador,
  deixando ~3 GB para o sistema. **Otimizações do Java**: G1GC com pausas curtas. As duas são gravadas na
  instância logo antes de abrir o jogo; desligadas, valem as configurações globais.
- Com o jogo aberto o launcher minimiza e entra em repouso: para de atualizar a lista, pausa carrossel e
  animações e consulta o progresso a cada 15 s. Volta ao normal quando o jogo fecha.
- **Presença no Discord** ("Jogando no <servidor>" com o botão **Jogar também**, que abre o convite):
  crie um aplicativo em https://discord.com/developers/applications, envie uma imagem chamada `conexao` (use `branding/icon-quadrado-1024.png`) em
  *Rich Presence → Art Assets* e defina `CONEXAO_DISCORD_APP_ID` no build do launcher. Sem essa variável a
  opção some (a presença do Modrinth no theseus foi desligada).

## Destaque pago

- Servidores **aprovados** compram destaque pelo painel (Visão geral), com Pix, cartão ou boleto pelo
  Checkout Pro do Mercado Pago. Os planos vêm de `CONEXAO_FEATURED_PLANS`.
- Ao confirmar o pagamento o destaque liga sozinho; comprar de novo soma os dias. Estorno ou chargeback
  descontam os dias.
- A API confirma cada pagamento consultando o Mercado Pago (`GET /v1/payments/{id}`), valida a assinatura do
  webhook (`x-signature`, HMAC-SHA256) e confere valor e moeda. Se o webhook falhar, a tela de retorno do
  painel concilia a compra.
- Configuração: crie a aplicação em Mercado Pago Developers, coloque o access token de produção em
  `CONEXAO_MP_ACCESS_TOKEN`, cadastre o webhook `https://<api>/v1/payments/mercadopago/webhook` (evento
  **Pagamentos**) e copie a assinatura secreta para `CONEXAO_MP_WEBHOOK_SECRET`.

## Desenvolvimento no Windows

Pré-requisitos:

- [Rust](https://rustup.rs) (o `rust-toolchain.toml` instala a versão certa)
- [Node.js 22](https://nodejs.org)
- JDK 21 (o `theseus` compila um pequeno JAR auxiliar com Gradle)
- Microsoft C++ Build Tools e WebView2 (já vem no Windows 11) — veja os
  [pré-requisitos do Tauri](https://v2.tauri.app/start/prerequisites/)

Launcher:

```powershell
cd apps\launcher
npm install
$env:CONEXAO_API_URL = "http://localhost:8080"   # API local
npx tauri dev
```

Instalador (`.exe`): `npx tauri build` — o arquivo sai em `target\release\bundle\nsis\`.

API local (precisa de um PostgreSQL rodando):

```powershell
$env:DATABASE_URL = "postgres://conexao:conexao@localhost/conexao"
$env:CONEXAO_JWT_SECRET = "uma-string-aleatoria-com-32-caracteres-ou-mais"
$env:CONEXAO_STAFF_TOKEN = "token-da-equipe"
cargo run -p conexao-api
```

Testes:

```powershell
cargo test -p conexao-types -p conexao-pack -p conexao-api -p conexao-agent
```

## Segurança dos modpacks

Os manifestos são assinados com Ed25519. Gere o par de chaves uma vez:

```bash
conexao-api gerar-chave-assinatura
# CONEXAO_SIGNING_KEY=...      → .env da VPS (segredo)
# CONEXAO_MANIFEST_PUBKEY=...  → variável do build do launcher (CI)
```

Com a chave pública embutida, o launcher recusa qualquer manifesto sem assinatura válida,
mesmo que alguém intercepte a conexão ou altere arquivos no servidor de arquivos.

## Atualização automática do launcher

1. Gere a chave do atualizador: `npx tauri signer generate -w conexao-updater.key`.
2. Cole a chave pública em `apps/launcher/src-tauri/tauri.release.conf.json`.
3. No GitHub, crie os secrets `TAURI_SIGNING_PRIVATE_KEY` (e a senha) e as variáveis
   `CONEXAO_API_URL`, `CONEXAO_MANIFEST_PUBKEY` e `CONEXAO_DISCORD_APP_ID` (opcional, ver abaixo).
4. **Assinatura de código do Windows** (Azure Trusted Signing): segredos `AZURE_CLIENT_ID`,
   `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` e variáveis `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`,
   `AZURE_SIGNING_PROFILE`. Sem eles o release falha; para um beta fechado, crie a variável
   `ALLOW_UNSIGNED_INSTALLER=true`.
5. Uma tag `v0.2.0` gera o instalador assinado e o `latest.json` num rascunho de release. O CI recusa
   o build se a chave do atualizador ainda for `COLE_AQUI...`, se o endpoint for o de exemplo
   (`api.conexao.gg`) ou se faltar `CONEXAO_API_URL`/`CONEXAO_MANIFEST_PUBKEY`. Um `tauri build`
   local também falha sem essas duas variáveis: o launcher de produção nunca sai sem a chave
   que confere os modpacks.
6. Copie o `latest.json` e os instaladores para a pasta `launcher/` do volume de arquivos
   da VPS. O launcher mostra a faixa **Atualizar e reiniciar** na próxima abertura.

## Segurança em produção

- **Modo produção**: com `CONEXAO_PUBLIC_URL` em https (fora de localhost), a API só sobe com
  `CONEXAO_SIGNING_KEY`, `CONEXAO_CORS_ORIGINS`, `CONEXAO_PANEL_URL` em https e, se houver pagamentos,
  `CONEXAO_MP_WEBHOOK_SECRET`. `CONEXAO_ALLOW_PRIVATE_PING` precisa estar desligado.
- **Limite de requisições** por IP (login, pareamento, Companion, detecção, criação de servidor e uploads
  têm limites próprios). Atrás do Caddy o IP vem do `X-Forwarded-For`.
- **Uploads**: no máximo `CONEXAO_MAX_UPLOAD_MB` (100) por arquivo, gravados em streaming, e cota diária por
  conta igual à cota do servidor. Cada conta pode ter até `CONEXAO_MAX_PENDING_SERVERS` (3) servidores em análise.
- **Imagens** (ícone, banner, fundo de perfil) são decodificadas e gravadas de novo: sai todo EXIF/GPS.
- **Detecção e ping de servidores** só conectam em IPs públicos já conferidos (SRV incluído).
- **Pareamento do painel**: o launcher mostra navegador, IP e horário do pedido antes de aprovar.
- **Token de publicação** (`conexao-publish`): vale 2 horas e só acessa as rotas de publicação.
- **Caddy**: HSTS, `nosniff`, CSP do painel, e `/files` sempre como download inerte.
- **Backup**: serviço `backup` (dump diário) e `offsite` (rclone); veja `docs/backup.md`.
- **Monitoramento**: `/health` testa banco e disco; log de acesso por requisição; aviso no Discord
  (`CONEXAO_ALERT_WEBHOOK`) para disco cheio e backup atrasado; veja `docs/monitoramento.md`.

## Deploy na VPS

```bash
cd deploy
cp .env.example .env    # preencha domínio, senhas e segredos
docker compose up -d --build
```

O Caddy emite o HTTPS, serve `/files/*` direto do disco com cache imutável e publica o
painel no `CONEXAO_PANEL_DOMAIN`. Antes de subir, gere o painel:

```bash
cd apps/panel && npm ci && VITE_CONEXAO_API_URL=https://api.conexao.gg npm run build
```

## Fluxo do administrador

1. Entrar no painel (código confirmado no launcher) e **Cadastrar servidor**.
2. A equipe Conexão aprova em **Equipe Conexão → Aprovações e destaques**.
3. Publicar o modpack na aba **Modpack**: **Selecionar pasta do modpack** → **Enviar e analisar**
   → revisar o diff, deixar arquivos de fora ou marcar configs para sempre sobrescrever →
   **Publicar**. O navegador calcula os hashes e envia só os arquivos novos; a análise usa as
   mesmas regras do `conexao-publish`.

   Para modpacks muito grandes ou automação, a linha de comando continua disponível:

   ```powershell
   $env:CONEXAO_API_URL = "https://api.conexao.gg"
   $env:CONEXAO_TOKEN = "<launcher → Token de publicação>"
   conexao-publish --server paravexia --dir "C:\caminho\do\modpack" --version 1.0.0
   ```

4. Na aba **Agente**, gerar o token e rodar `conexao-agent --config conexao-agent.toml` no servidor.

## Créditos e licença

O Conexão é software livre sob a **GNU GPL v3** (veja `LICENSE`). A pasta `vendor/`
contém código do [Modrinth](https://github.com/modrinth/code): `theseus` (GPL-3.0),
`daedalus` (MIT) e bibliotecas auxiliares, com seus avisos de licença originais.
A marca e o logo do Modrinth **não** são usados neste projeto.

# Parceria Central SMP's

A Central SMP's (comunidade de SMPs no Discord) é a parceira oficial do Conexão. Este documento
explica o que foi implementado e como configurar.

## Como alguém vira "Membro da Central"

1. A pessoa abre o link **`https://conexaosmps.com.br/?ref=central`** (é o link para divulgar
   na Central). O site guarda a origem e a repassa ao login do Discord
   (`/v1/waitlist/discord?ref=central`).
2. Com `CONEXAO_CENTRAL_GUILD_ID` configurado, o login do Discord pede `identify guilds`.
   A API lê só a lista de servidores da pessoa e confere se o da Central está nela.
3. Resultado na lista de espera: `ref = 'central'` (de onde veio) e `central_member`
   (verificado). **Só o verificado ganha o selo e entra no lote prioritário**: copiar o link
   não basta.
4. Quando a conta entra no launcher (Microsoft ou Discord), a origem e o selo passam para
   `users.central_member` / `users.signup_ref`. Cada novo login pelo Discord confere de novo:
   quem saiu da Central perde o selo; se o Discord não responder, o selo fica como estava.

## Selos (roles)

| Selo | Quem ganha | Onde aparece |
| --- | --- | --- |
| **Membro da Central** | Membro verificado da Central | Perfil (card, página pública), menu da conta no launcher |
| **SMP da Central** | Servidores marcados pela equipe no programa + a equipe deles | Card e página do servidor no launcher, filtro "SMPs da Central", faixa na home, perfil da equipe |

A equipe Conexão marca o servidor em **Painel → Equipe Conexão → Todos os servidores →
coluna Central** (`POST /v1/staff/servers/{slug}/central`).

## Lista de espera

**Painel → Equipe Conexão → Lista de espera**:

- selo "Central" (verificado) ou "via central" (só o link) em cada pedido;
- filtro "Só membros da Central";
- **Liberar lote da Central**: libera os próximos N membros verificados, por ordem de chegada
  (`POST /v1/staff/waitlist/next` com `{"count": N, "central": true}`).

## Site

- Faixa no topo, selo "× Central SMP's" ao lado do logo, "com a Central SMP's" no hero e no
  rodapé.
- Seção **#central** com os dois logos, benefícios e os botões "Entrar na lista pela Central"
  e "Entrar na Central SMP's" (este só aparece com `CONEXAO_CENTRAL_INVITE`).
- Vindo pelo `?ref=central`: aviso no quadro da lista; já verificado: selo "Membro da Central
  verificado · lote prioritário" na posição da fila.

## Configuração

No `deploy/.env` da VPS:

```
CONEXAO_CENTRAL_GUILD_ID=<ID do servidor da Central>
CONEXAO_CENTRAL_INVITE=https://discord.gg/<convite da Central>
```

Depois `docker compose up -d api`. A migração `0014_central.sql` roda sozinha. Não precisa
mudar nada no app do Discord: o escopo `guilds` não pede aprovação.

## Página do servidor: imagens

Junto com a parceria, a página PRO do servidor mudou:

- **Imagens só sobem ao salvar.** Escolher um arquivo cria só uma prévia no navegador. Ao
  salvar, o painel confere a página (`POST /v1/servers/{slug}/page/check`, com `pending:N` no
  lugar das imagens novas e o tamanho de cada arquivo), e só se passar envia os arquivos e grava.
- **Imagens guardadas** (nova seção no painel): lista tudo o que foi enviado
  (`GET /v1/servers/{slug}/page/media`), com "Usar" (sem reenviar) e "Apagar"
  (`DELETE /v1/servers/{slug}/page/media/{id}`). Imagem que está na página salva precisa sair do
  bloco antes. O arquivo sai do disco se nada mais usa o mesmo arquivo.
- **Grade por cima do gradiente**: no launcher (e na prévia do painel) as linhas em quadrados
  ficam por cima do fundo personalizado, na cor do tema (`gridColor` em `lib/page.ts`).

## Prévias de divulgação

`apps/launcher/preview/` roda o launcher no navegador com dados fictícios
(`npm run preview:mock`). Veja o README da pasta. Prints prontos em `branding/previews/`.

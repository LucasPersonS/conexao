# Página do servidor (PRO) e formulário de whitelist

## Página personalizada (plano PRO)

No painel, aba **Página** (dono e admins). Aparece no launcher, na página do servidor,
logo abaixo de "Sobre o servidor".

- **Fundo**: gradiente linear ou radial com 2 a 4 cores (8 modelos prontos). O launcher
  escurece um pouco o fundo para o texto continuar legível.
- **Cor de destaque**: títulos dos blocos, borda do ícone e botão de play dos vídeos.
- **Blocos** (até 20, em qualquer ordem):
  - **Texto**: título (80) e texto (3.000 caracteres), passa pelo filtro de palavras.
  - **Imagem ou GIF**: clique para ampliar no launcher.
  - **Galeria**: 2 a 12 imagens/GIFs, com navegação por setas.
  - **Vídeo do YouTube**: cola o link (watch, youtu.be, shorts, live, embed); a API guarda
    só o ID. O launcher mostra a capa e só carrega o player do `youtube-nocookie.com`
    depois do clique. Também tem "Abrir no YouTube" (abre no navegador).
- A prévia ao lado do editor mostra como fica no launcher.

**PRO venceu**: a página some do launcher, fica guardada (a equipe ainda vê no painel) e
volta como estava ao renovar. Sem PRO não dá para salvar nem enviar imagens (a equipe
Conexão pode).

### Envio de imagens

- PNG, JPEG ou WebP até 5 MB: regravadas sem metadados (igual ao ícone e ao banner).
- GIF animado até 8 MB, no máximo 1600×1600 e 600 quadros: a API regrava bloco a bloco
  (mantém quadros, tempo e laço; tira comentários, XMP e qualquer coisa depois do fim do
  arquivo) e decodifica tudo antes de aceitar.
- Por servidor: 40 envios e 120 MB a cada 24 h.
- Só aceita na página imagens enviadas para aquele servidor (`/images/{sha1}.ext` do próprio
  Conexão). Nada de link externo: o launcher não carrega imagem de terceiros.
- Limpeza diária: imagem que saiu da página há mais de 48 h perde o registro e sai do disco
  se nada mais usa o arquivo (ícone, banner, outra página, patrocinador ou fundo de perfil).

### Onde está

- Migração `apps/api/migrations/0013_pagina_servidor.sql` (`servers.page`,
  `servers.whitelist_form_url`, `server_page_media`).
- API `apps/api/src/routes/page.rs`: `GET/PUT /v1/servers/{slug}/page`,
  `POST /v1/servers/{slug}/page/media`. `ServerSummary.has_page` diz ao launcher se vale buscar.
- Launcher: `ServerShowcase.vue`, `lib/page.ts`; fundo aplicado em `App.vue`. Rota liberada
  em `route_allowed` (só `GET .../page`). CSP: `frame-src https://www.youtube-nocookie.com`.
- Painel: `PageTab.vue`.

## Formulário para pedir whitelist (todos os servidores)

No painel → **Visão geral**, com "Exigir whitelist" ligado: campo **Formulário para pedir
whitelist**. Só aceita Google Forms (`https://docs.google.com/forms/...` ou
`https://forms.gle/...`). Deixe vazio para remover.

No launcher, quem ainda não tem whitelist (sem pedido, pendente ou recusado) vê o quadro
**Como pedir whitelist** embaixo do botão Jogar, com **Preencher formulário** e **Entrar no
Discord** (usa o link do Discord do servidor). O botão "Pedir whitelist" continua: é ele que
coloca o jogador na fila da aba Whitelist do painel.

**Formulário pré-preenchido**: no Google Forms, "Gerar link pré-preenchido", escreva `{nick}`
na pergunta do nick e cole o link. O launcher troca `{nick}` pelo nick do jogador.

## Lançamento

1. Atualize a VPS (a migração 0013 roda sozinha).
2. Lance o launcher **0.1.92** (a lista de rotas e a CSP do Tauri mudaram). O
   `tauri.conf.json` estava em 0.1.6 desde as tags v0.1.7–v0.1.91; agora acompanha a tag.
   Launchers antigos continuam funcionando, só não mostram a página nem o formulário.

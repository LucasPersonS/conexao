# Patrocinadores no launcher

Banners clicáveis cadastrados pela equipe Conexão em **Painel → Equipe Conexão → Patrocinadores**.

## Onde aparecem

| Posição | Onde | Imagem usada |
|---|---|---|
| **Home** | Faixa larga abaixo do topo da tela inicial | Banner largo **8:1** (ex.: 1600×200) |
| **Página do servidor** | Coluna lateral de todos os servidores | Banner lateral **2:1** (ex.: 800×400) ou o largo |
| **Carregamento do jogo** | Embaixo do progresso enquanto baixa e abre o Minecraft | Banner lateral 2:1 ou o largo |

Com mais de um patrocinador na mesma posição, os banners trocam sozinhos (8 s na home e no
servidor, 10 s no carregamento). O **peso** (1 a 10) define quem aparece primeiro e com mais
frequência. A rotação para com o jogo aberto, com a janela escondida ou com o mouse em cima.

## Cadastro

- Nome, link (só `https://`), descrição curta (acessibilidade), imagens, posições, peso,
  início e fim (vazio = agora / sem fim) e ativo/pausado.
- Imagens PNG, JPEG ou WebP até 5 MB. A API regrava a imagem e apaga metadados (EXIF/GPS);
  SVG e arquivos disfarçados são recusados. Só imagens enviadas ao próprio Conexão são aceitas.
- Dica: use links com UTM (`?utm_source=conexao&utm_medium=launcher`) para o patrocinador ver
  os acessos no Google Analytics dele.

## Números para o patrocinador

- **Visualização:** o banner apareceu na tela com o launcher visível.
- **Clique:** o jogador clicou no banner.
- Cada jogador conta **uma vez por dia em cada posição** (para visualização e para clique), só com
  login e só enquanto o banner está no ar naquela posição. Recarregar ou ficar parado na tela não
  infla os números.
- O painel mostra os totais de 30 dias e a taxa de clique (CTR). Em *Relatório* há o dia a dia por
  posição e o botão **Baixar planilha (CSV)** para mandar ao patrocinador.

## Rotas da API

| Rota | Quem |
|---|---|
| `GET /v1/sponsors/{home\|server\|loading}` | público (lista o que está no ar) |
| `POST /v1/sponsors/events` `{placement, views[], click?}` | jogador logado |
| `GET/POST /v1/staff/sponsors`, `PUT/DELETE /v1/staff/sponsors/{id}` | equipe |
| `POST /v1/staff/sponsors/image` | equipe (envio da imagem) |
| `GET /v1/staff/sponsors/{id}/stats` | equipe (30 dias, por dia e posição) |

O launcher só chama as duas primeiras (lista fechada em `route_allowed`). A migração
`0012_patrocinadores.sql` roda sozinha ao subir a API.

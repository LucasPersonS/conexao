# Conexão Companion no Conexão

O código do mod fica no repositório **`conexao-companion`** (Architectury: `common/`, `fabric/`, `forge/`,
`neoforge/`). Aqui ficam só a integração: API, painel e deploy.

## Como os JARs chegam ao painel

1. Tag `v<versão>` no `conexao-companion` → o Actions compila todas as versões × loaders e cria o release com
   `conexao-companion-<loader>-<mc>.jar` + `manifest.json`.
2. `deploy/atualizar.sh` baixa o release para `<storage>/launcher/companion/` (o Caddy serve em
   `https://api.<domínio>/launcher/companion/`, sem cache).
3. A API lê o `manifest.json`:
   - `GET /v1/companion/builds` → `{ version, builds: [{ minecraft, supports, loader, java, file, size, sha1, url }] }`
     (público; o painel usa para o botão de download);
   - `GET /v1/companion/download?minecraft=1.20.1&loader=forge` → redireciona para o JAR certo (links diretos).
4. Painel, aba **Servidor**: o botão "Baixar para <loader> <versão>" usa a versão e o loader que o Companion
   informou ou, antes do vínculo, os do cadastro do servidor. "Outra versão ou loader" lista todos os JARs.
   Servidor com Companion mais antigo que o publicado vê o aviso de atualização.

Escolha do JAR (`pickCompanionBuild` no painel, `companion_builds::pick` na API): mesmo loader; primeiro a
versão de compilação exata, depois a faixa `supports` (no Fabric um JAR cobre, por exemplo, 1.21–1.21.1).

## Desenvolvimento local

Sem release publicado, copie os JARs do `dist/` do `conexao-companion` (com o `manifest.json`) para
`<CONEXAO_STORAGE_DIR>/launcher/companion/` e recarregue o painel.

A pasta `companion/` antiga (só NeoForge 1.21.1) foi substituída pelo repositório novo.

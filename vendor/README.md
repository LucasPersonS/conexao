# Código de terceiros

Copiado de [modrinth/code](https://github.com/modrinth/code) (commit `2124212a44703f874fcf6039212973784b0f50e9`, outubro de 2026).

| Pasta | Origem | Licença |
| --- | --- | --- |
| `theseus/` | `packages/app-lib` | GPL-3.0-only (veja `theseus/COPYING.md`) |
| `daedalus/` | `packages/daedalus` | MIT (`daedalus/LICENSE`) |
| `async-minecraft-ping/` | `packages/async-minecraft-ping` | MIT ou Apache-2.0 |
| `ariadne/`, `modrinth-content-management/`, `path-util/`, `serde-binhum/` | `packages/*` | GPL-3.0-only (parte do monorepo do Modrinth) |

Alterações feitas no Conexão:

- `theseus/src/api/minecraft_auth.rs`: nova função pública `get_default_credentials()`
  (o launcher precisa do access token para abrir a sessão no Conexão).
- `theseus/build.rs`: roda `gradlew assemble` em vez de `build` (o spotless falhava no Windows
  por causa das quebras de linha CRLF).
- `theseus/src/util/fetch.rs`: `https_only` só em builds de release (em dev a API local é `http://`).
- `async-minecraft-ping/src/server.rs`: expõe o JSON bruto do status (`raw`) e aceita MOTD
  sem `text` (usado pela detecção automática do painel).
- `theseus/.env` copiado de `.env.prod` (URLs públicas de metadados do Modrinth).
  Na fase 2 do roadmap o `MODRINTH_LAUNCHER_META_URL` passa a apontar para o mirror
  de metadados do Conexão.

Para atualizar: copie as pastas de uma versão nova do monorepo, mantenha as
alterações acima e rode `cargo check -p theseus`.

A marca Modrinth (logo, nome, imagens) não pode ser usada sem permissão da Rinth, Inc.

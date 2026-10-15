# Conexão Companion

Mod **só de servidor** (os jogadores não precisam dele) que liga o servidor ao Conexão sem configuração:

1. O dono coloca `conexao-companion-neoforge-1.21.1-<versão>.jar` na pasta `mods/` e reinicia.
2. O console mostra um link (`https://painel.../link/K7Q2-9XMA`). O dono abre, escolhe o servidor e confirma.
3. Pronto: o mod guarda o token em `config/conexao-companion.json` e passa a:
   - aplicar a whitelist aprovada no painel na hora (sem RCON; entradas manuais do dono são preservadas);
   - ligar/desligar a whitelist conforme "Exigir whitelist" no painel;
   - informar versões exatas (Minecraft, NeoForge) e os mods instalados;
   - enviar ao Conexão, quando pedido, os mods que não existem no Modrinth.

No painel, a aba **Servidor** mostra o status e avisa quando os mods do servidor mudaram; a aba **Modpack**
monta a nova versão com **Montar a partir do servidor**.

## Build

```bash
cd companion
CONEXAO_API_URL=https://api.conexao.gg gradle build   # sem a variável, usa a API de produção
# saída: build/libs/conexao-companion-neoforge-1.21.1-<versão>.jar
```

Publique o JAR em `<storage>/launcher/companion/conexao-companion-neoforge-1.21.1.jar` na VPS: o painel
aponta o botão de download para esse caminho.

Para testar localmente: `CONEXAO_API_URL=http://127.0.0.1:8080 gradle runServer` (pasta `run/`).

## Próximos alvos

Forge e Fabric, e versões 1.18.2–1.21.x/26.x (mesma lógica; muda só a camada do loader).

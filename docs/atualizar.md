# Atualizar a produção com um comando

## Uma vez só: token do GitHub na VPS

1. GitHub → foto → **Settings → Developer settings → Personal access tokens → Fine-grained tokens → Generate new token**.
2. Nome `vps-conexao`, validade de 1 ano, **Only select repositories → conexao**.
3. Permissões: **Contents: Read-only** (o resto fica sem acesso).
4. Na VPS:
   ```
   nano /opt/conexao/deploy/.github-token   # cole o token, salve
   chmod 600 /opt/conexao/deploy/.github-token
   ```

## Cada atualização

| Mudou | No seu PC | Na VPS |
|---|---|---|
| Painel ou API | `git push` | `bash /opt/conexao/deploy/atualizar.sh` |
| Launcher | aumente `version` em `apps/launcher/src-tauri/tauri.conf.json`, `git push`, `git tag v0.1.1`, `git push origin v0.1.1`, espere o Actions ficar verde | `bash /opt/conexao/deploy/atualizar.sh` |

O script baixa a `main`, refaz só o que mudou (painel e/ou API), reinicia, baixa o último
release publicado, troca os links do `latest.json` para `https://api.<domínio>/launcher/` e
confere o `/health`. Rodar de novo sem mudanças não faz nada. `.env`, `rclone.conf` e backups
nunca são tocados.

# Backup do Conexão

## O que é guardado

| O quê | Onde fica na VPS | Como é copiado |
| --- | --- | --- |
| Banco (contas, servidores, versões, pagamentos) | volume `pgdata` | serviço `backup`: um `pg_dump` por dia em `deploy/backups/db` (14 dias) |
| Arquivos dos modpacks, imagens, instaladores | volume `files` | serviço `offsite` (rclone), junto com os dumps |

O serviço `backup` sobe com `docker compose up -d`. Um backup que só existe na VPS não
protege contra perder a VPS: ligue também a cópia externa.

## Cópia externa (rclone)

1. Crie um bucket (Backblaze B2, Cloudflare R2, Amazon S3...) e uma chave de acesso.
2. Na VPS, dentro de `deploy/`: `docker run --rm -it -v "$PWD:/config/rclone" rclone/rclone config`
   e crie um remoto chamado `backup` (o arquivo fica em `deploy/rclone.conf`).
3. No `.env`: `RCLONE_REMOTE=backup:nome-do-bucket`.
4. `docker compose --profile offsite up -d`. O log mostra "cópia externa ok" uma vez por dia.

## Restaurar (teste uma vez por mês)

```bash
cd deploy
docker compose stop api
docker compose exec -T postgres pg_restore --clean --if-exists --no-owner \
  -U "${POSTGRES_USER:-conexao}" -d "${POSTGRES_DB:-conexao}" < backups/db/conexao-AAAAMMDD-HHMM.dump
docker compose start api
```

Para os arquivos: `rclone copy backup:nome-do-bucket/files <volume files>` (o caminho do volume
aparece em `docker volume inspect deploy_files`).

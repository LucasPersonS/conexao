# Monitoramento do Conexão

## O que já vem pronto

| O quê | Como funciona |
| --- | --- |
| Saúde da API | `https://api.seudominio/health` responde `ok` (200). Responde 503 se o banco cair ou se o disco passar de 90% (`CONEXAO_DISK_ALERT_PERCENT`). |
| Aviso no Discord | Com `CONEXAO_ALERT_WEBHOOK`, a API avisa quando o disco passa de 85% (`CONEXAO_DISK_WARN_PERCENT`) e quando o backup do banco fica mais de 26 h sem rodar. Repete a cada 6 h enquanto durar e avisa quando normaliza. |
| Log de acesso | Uma linha por requisição: método, caminho, status, tempo e IP. Ver com `docker compose logs -f api`. |
| Tamanho dos logs | Cada contêiner guarda no máximo 3 arquivos de 10 MB de log. |

## UptimeRobot (aviso de site fora do ar)

1. Crie a conta em uptimerobot.com (o plano gratuito basta).
2. **Add New Monitor** → tipo **HTTP(s)**:
   - Nome `Conexão API`, URL `https://api.seudominio/health`, intervalo 5 minutos.
   - Nome `Conexão Painel`, URL `https://painel.seudominio`, intervalo 5 minutos.
3. Em **Alert Contacts**, cadastre seu e-mail (e, se quiser, o Discord ou o Telegram) e marque nos dois monitores.
4. Teste: `docker compose stop api` na VPS. Em até 5 minutos chega o aviso. Depois `docker compose start api`.

## Aviso direto no Discord

1. No canal da equipe: **Editar canal → Integrações → Webhooks → Novo webhook → Copiar URL**.
2. No `.env` da VPS: `CONEXAO_ALERT_WEBHOOK=<url copiada>`.
3. `docker compose up -d` para aplicar.

## Comandos úteis na VPS

```bash
docker compose logs -f api            # acessos e erros da API
docker compose logs --since 1h api | grep " 5[0-9][0-9] "   # erros 5xx da última hora
docker compose logs backup            # resultado dos backups
df -h                                 # espaço em disco
```

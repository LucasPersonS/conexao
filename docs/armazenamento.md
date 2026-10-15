# Armazenamento dos arquivos dos modpacks

O que fica no disco da VPS são só os arquivos que **não** existem no Modrinth ou no
CurseForge (configs, mods próprios, resource packs). O resto o launcher baixa direto de lá.
Cada arquivo é guardado uma vez, pelo SHA-1, mesmo que apareça em várias versões ou servidores.

## Limites (deploy/.env)

| Variável | Padrão | O que limita |
|---|---|---|
| `CONEXAO_MAX_UPLOAD_MB` | 100 | tamanho de cada arquivo |
| `CONEXAO_SERVER_QUOTA_MB` | 300 | arquivos hospedados por versão, e envio por conta a cada 24 h |
| `CONEXAO_FEATURED_QUOTA_MB` | 1024 | o mesmo, para servidores em destaque |

Servidor **em análise**: todas as versões juntas cabem na cota (não dá para acumular
versões enquanto a equipe não aprova).

## Trava de disco

Com o disco acima de `CONEXAO_DISK_ALERT_PERCENT` (90%), a API recusa arquivos novos
(“armazenamento cheio, tente mais tarde”). O monitor já avisa no Discord a partir de 85%.

## Limpeza diária (apps/api/src/cleanup.rs)

Roda 15 minutos depois da API subir e depois a cada 24 h:

1. apaga as versões de servidores **recusados há mais de 7 dias**;
2. apaga arquivos que nenhuma versão publicada usa e que ninguém enviou ou consultou
   nas últimas 48 h (envios abandonados, servidores apagados ou recusados);
3. apaga temporários de envios interrompidos com mais de 24 h.

Versões de servidores aprovados, em análise ou suspensos nunca perdem arquivos: o rollback
para versões antigas continua funcionando. O resultado aparece no log da API
(`docker compose logs api | grep limpeza`).

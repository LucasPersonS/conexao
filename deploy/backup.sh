#!/bin/sh
# Backup diário do PostgreSQL (roda no contêiner "backup" do docker-compose).
# Guarda um dump por dia em ./backups/db e apaga os mais velhos que BACKUP_KEEP_DAYS.
set -eu
KEEP="${BACKUP_KEEP_DAYS:-14}"
HOUR="${BACKUP_HOUR_UTC:-6}"
DIR=/backups/db
mkdir -p "$DIR"

dump() {
  file="$DIR/conexao-$(date -u +%Y%m%d-%H%M).dump"
  if pg_dump --format=custom --no-owner --file="$file.tmp"; then
    mv "$file.tmp" "$file"
    echo "backup ok: $file ($(du -h "$file" | cut -f1))"
  else
    rm -f "$file.tmp"
    echo "ERRO no backup do banco" >&2
  fi
  find "$DIR" -name 'conexao-*.dump' -mtime +"$KEEP" -delete
}

# Na subida: se não há backup de hoje, faz um agora.
ls "$DIR"/conexao-"$(date -u +%Y%m%d)"-*.dump >/dev/null 2>&1 || dump

while true; do
  now=$(date -u +%s)
  next=$(date -u -d "$(date -u +%Y-%m-%d) $HOUR:00" +%s)
  [ "$next" -le "$now" ] && next=$((next + 86400))
  sleep $((next - now))
  dump
done

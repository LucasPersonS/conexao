#!/usr/bin/env bash
# Instala o Conexão numa VPS nova (Ubuntu 22.04/24.04 ou Debian 12), como root.
#
# Uso (dentro da pasta deploy/ do projeto, na VPS):
#   bash instalar-vps.sh conexaosmps.com.br SEU-UUID-MINECRAFT
#
# O que faz:
#   1. atualiza o sistema, liga atualizações automáticas, firewall (22, 80, 443) e fail2ban;
#   2. instala o Docker;
#   3. cria o deploy/.env com senhas e segredos novos (só se ainda não existir);
#   4. gera o painel web e a imagem da API;
#   5. gera a chave de assinatura dos modpacks e mostra a chave pública para o GitHub;
#   6. sobe tudo (site, API e painel; o Caddy emite o HTTPS sozinho) e confere o /health.
# Pode rodar de novo sem medo: não recria o .env nem troca chaves que já existem.
set -euo pipefail

DOMINIO="${1:-}"
STAFF_UUID="${2:-}"
cd "$(dirname "$0")"

erro() { echo -e "\n\033[31mERRO:\033[0m $*" >&2; exit 1; }
passo() { echo -e "\n\033[1;32m==>\033[0m \033[1m$*\033[0m"; }

[ "$(id -u)" -eq 0 ] || erro "rode como root (sudo bash instalar-vps.sh ...)"
[ -n "$DOMINIO" ] || erro "informe o domínio: bash instalar-vps.sh conexaosmps.com.br SEU-UUID-MINECRAFT"
[ -f docker-compose.yml ] || erro "rode este script dentro da pasta deploy/ do projeto"

# Domínio com acento (ex.: conexãosmps.com.br) vira a forma ASCII (xn--...), a única que
# certificados HTTPS e programas aceitam sem surpresa.
DOMINIO_ASCII=$(python3 -c 'import sys; print(sys.argv[1].strip().lower().encode("idna").decode())' "$DOMINIO")
API="api.$DOMINIO_ASCII"
PAINEL="painel.$DOMINIO_ASCII"

passo "Atualizando o sistema"
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get -y -q upgrade
apt-get -y -q install ca-certificates curl git ufw fail2ban unattended-upgrades python3 dnsutils
dpkg-reconfigure -f noninteractive unattended-upgrades

passo "Firewall: só SSH (22), HTTP (80) e HTTPS (443)"
ufw allow OpenSSH >/dev/null
ufw allow 80/tcp >/dev/null
ufw allow 443/tcp >/dev/null
ufw --force enable >/dev/null
systemctl enable --now fail2ban >/dev/null
ufw status | sed 's/^/   /'

if ! command -v docker >/dev/null; then
  passo "Instalando o Docker"
  curl -fsSL https://get.docker.com | sh
fi
systemctl enable --now docker >/dev/null

# Lê e grava uma variável do .env sem mostrar o valor na tela.
get_env() { grep -E "^$1=" .env | tail -n1 | cut -d= -f2- || true; }
set_env() {
  if grep -qE "^$1=" .env; then
    python3 - "$1" "$2" <<'PY'
import sys
key, value = sys.argv[1], sys.argv[2]
lines = open('.env', encoding='utf-8').read().split('\n')
lines = [f'{key}={value}' if l.startswith(key + '=') else l for l in lines]
open('.env', 'w', encoding='utf-8').write('\n'.join(lines))
PY
  else
    printf '%s=%s\n' "$1" "$2" >> .env
  fi
}
segredo() { head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c "$1"; }

if [ ! -f .env ]; then
  passo "Criando o .env com segredos novos"
  cp .env.example .env
  chmod 600 .env
  set_env POSTGRES_PASSWORD "$(segredo 40)"
  set_env CONEXAO_JWT_SECRET "$(segredo 64)"
  set_env CONEXAO_STAFF_TOKEN "$(segredo 48)"
  set_env RCLONE_REMOTE ""
fi
set_env CONEXAO_DOMAIN "$API"
set_env CONEXAO_PANEL_DOMAIN "$PAINEL"
set_env CONEXAO_SITE_DOMAIN "$DOMINIO_ASCII"
set_env CONEXAO_PROFILE_DOMAIN "perfil.$DOMINIO_ASCII"
set_env CONEXAO_CORS_ORIGINS "https://$PAINEL"
[ -n "$STAFF_UUID" ] && set_env CONEXAO_STAFF_UUIDS "$STAFF_UUID"
chmod 600 .env

passo "Conferindo o DNS"
IP_VPS=$(curl -fsS4 https://api.ipify.org || true)
for nome in "$API" "$PAINEL" "$DOMINIO_ASCII" "www.$DOMINIO_ASCII" "perfil.$DOMINIO_ASCII"; do
  aponta=$(dig +short A "$nome" | tail -n1)
  if [ "$aponta" = "$IP_VPS" ]; then
    echo "   $nome -> $aponta (ok)"
  else
    echo "   $nome -> ${aponta:-nada} (esperado: $IP_VPS)"
    echo "   Crie o registro A no seu DNS (Cloudflare: "Somente DNS", nuvem cinza). Sem isso o HTTPS não é emitido; rode o script de novo depois."
  fi
done

passo "Gerando o painel web"
docker run --rm -v "$(cd .. && pwd)":/src -w /src/apps/panel \
  -e VITE_CONEXAO_API_URL="https://$API" node:22-bookworm-slim \
  sh -c "npm ci --no-audit --no-fund && npm run build"

passo "Gerando a imagem da API (a primeira vez leva de 10 a 20 minutos)"
docker compose build api

if [ -z "$(get_env CONEXAO_SIGNING_KEY)" ]; then
  passo "Gerando a chave de assinatura dos modpacks"
  saida=$(docker compose run --rm --no-deps -T api conexao-api gerar-chave-assinatura)
  privada=$(echo "$saida" | grep '^CONEXAO_SIGNING_KEY=' | cut -d= -f2- | awk '{print $1}')
  publica=$(echo "$saida" | grep '^CONEXAO_MANIFEST_PUBKEY=' | cut -d= -f2- | awk '{print $1}')
  [ -n "$privada" ] && [ -n "$publica" ] || erro "não consegui gerar a chave de assinatura"
  set_env CONEXAO_SIGNING_KEY "$privada"
  echo "$publica" > CHAVE_PUBLICA_LAUNCHER.txt
fi

passo "Subindo o Conexão"
mkdir -p backups
docker compose up -d

echo -n "   Esperando a API responder"
for _ in $(seq 1 60); do
  if docker compose exec -T api curl -fsS http://localhost:8080/health >/dev/null 2>&1; then
    echo " ok"; break
  fi
  echo -n "."; sleep 3
done
docker compose ps

passo "Pronto"
cat <<FIM
   Site:   https://$DOMINIO_ASCII
   API:    https://$API/health
   Painel: https://$PAINEL
   Perfis: https://perfil.$DOMINIO_ASCII/u/<nick>
   (o HTTPS pode levar 1 ou 2 minutos depois que o DNS estiver apontando)

   No GitHub (Settings > Secrets and variables > Actions > Variables), crie:
     CONEXAO_API_URL          = https://$API
     CONEXAO_MANIFEST_PUBKEY  = $(cat CHAVE_PUBLICA_LAUNCHER.txt 2>/dev/null || echo "veja deploy/CHAVE_PUBLICA_LAUNCHER.txt")

   Guarde uma cópia do deploy/.env num gerenciador de senhas: sem ele não dá para
   recuperar a chave de assinatura nem a senha do banco.
FIM

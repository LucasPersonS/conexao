#!/usr/bin/env bash
# Atualiza o Conexão na VPS com um comando só (rode como root, na VPS):
#
#   bash /opt/conexao/deploy/atualizar.sh
#
# O que faz:
#   1. baixa o código mais novo da branch main do GitHub;
#   2. gera de novo o painel web e a imagem da API e reinicia (só o que mudou);
#   3. baixa o instalador do último release publicado e ajusta o latest.json para
#      apontar para https://api.<domínio>/launcher/ (o /baixar e a atualização
#      automática do launcher passam a entregar a versão nova);
#   4. confere o /health.
#
# Pré-requisito (uma vez só): um token do GitHub só de leitura em deploy/.github-token
# (veja docs/atualizar.md). O .env, o rclone.conf e os backups nunca são tocados.
set -euo pipefail

REPO="${CONEXAO_REPO:-LucasPersonS/conexao}"
BRANCH="${CONEXAO_BRANCH:-main}"
cd "$(dirname "$0")"
DEPLOY="$(pwd)"
RAIZ="$(cd .. && pwd)"

erro() { echo -e "\n\033[31mERRO:\033[0m $*" >&2; exit 1; }
passo() { echo -e "\n\033[1;32m==>\033[0m \033[1m$*\033[0m"; }

[ "$(id -u)" -eq 0 ] || erro "rode como root"
[ -f .env ] || erro "não achei deploy/.env (rode antes o instalar-vps.sh)"
[ -s .github-token ] || erro "crie o token do GitHub em $DEPLOY/.github-token (veja docs/atualizar.md)"
chmod 600 .github-token
TOKEN="$(tr -d '[:space:]' < .github-token)"
API_DOMAIN="$(grep -E '^CONEXAO_DOMAIN=' .env | tail -n1 | cut -d= -f2-)"
[ -n "$API_DOMAIN" ] || erro "CONEXAO_DOMAIN vazio no .env"

# Git sem gravar o token em disco: ele vai só no cabeçalho desta execução.
AUTH="AUTHORIZATION: basic $(printf 'x-access-token:%s' "$TOKEN" | base64 -w0)"
git_() { git -C "$RAIZ" -c http.extraheader="$AUTH" "$@"; }

passo "Baixando o código novo ($REPO, $BRANCH)"
command -v git >/dev/null || apt-get install -y -q git
# A pasta veio do seu PC (outro dono); o git só aceita mexer nela com esta exceção.
git config --global --get-all safe.directory | grep -qx "$RAIZ" || git config --global --add safe.directory "$RAIZ"
if [ ! -d "$RAIZ/.git" ]; then
  # Primeira vez: a pasta veio por cópia, sem git. Liga ao repositório sem apagar
  # nada que não está no git (.env, rclone.conf, backups, chaves).
  git -C "$RAIZ" init -q -b "$BRANCH"
fi
git -C "$RAIZ" remote get-url origin >/dev/null 2>&1 \
  || git -C "$RAIZ" remote add origin "https://github.com/$REPO.git"
ANTES="$(git -C "$RAIZ" rev-parse -q --verify HEAD || echo nenhum)"
git_ fetch -q origin "$BRANCH"
git -C "$RAIZ" reset -q --hard "origin/$BRANCH"
DEPOIS="$(git -C "$RAIZ" rev-parse HEAD)"
echo "   $ANTES -> $DEPOIS"

mudou() {
  [ "$ANTES" = nenhum ] && return 0
  [ -n "$(git -C "$RAIZ" diff --name-only "$ANTES" "$DEPOIS" -- "$@")" ]
}

if mudou apps/panel; then
  passo "Gerando o painel web"
  docker run --rm -v "$RAIZ":/src -w /src/apps/panel \
    -e VITE_CONEXAO_API_URL="https://$API_DOMAIN" node:22-bookworm-slim \
    sh -c "npm ci --no-audit --no-fund && npm run build"
else
  echo "   painel sem mudanças"
fi

if mudou apps/api Cargo.toml Cargo.lock deploy/Dockerfile.api; then
  passo "Gerando a imagem da API"
  DOCKER_BUILDKIT=1 docker compose build api
else
  echo "   API sem mudanças"
fi

passo "Reiniciando os serviços"
docker compose up -d --remove-orphans
docker compose restart caddy >/dev/null   # pega o painel novo e mudanças no Caddyfile

passo "Instalador do launcher (último release publicado)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
gh_api() { curl -fsSL -H "Authorization: Bearer $TOKEN" -H "X-GitHub-Api-Version: 2022-11-28" "$@"; }
if gh_api -H "Accept: application/vnd.github+json" \
     "https://api.github.com/repos/$REPO/releases/latest" > "$TMP/release.json" 2>/dev/null; then
  python3 - "$TMP/release.json" > "$TMP/assets.txt" <<'PY'
import json, sys
rel = json.load(open(sys.argv[1]))
print(rel["tag_name"])
for a in rel["assets"]:
    n = a["name"]
    if n == "latest.json" or n.endswith((".exe", ".exe.sig", ".msi", ".msi.sig")):
        print(a["url"], n)
PY
  TAG="$(head -n1 "$TMP/assets.txt")"
  ATUAL="$(docker compose exec -T api sh -c 'cat /data/launcher/.versao 2>/dev/null' || true)"
  if [ "$TAG" = "$ATUAL" ]; then
    echo "   $TAG já está publicado"
  else
    tail -n +2 "$TMP/assets.txt" | while read -r url nome; do
      echo "   baixando $nome"
      gh_api -H "Accept: application/octet-stream" -o "$TMP/$nome" "$url"
    done
    [ -f "$TMP/latest.json" ] || erro "o release $TAG não tem latest.json"
    # Links do GitHub (privados) -> arquivos servidos pela própria API.
    python3 - "$TMP/latest.json" "https://$API_DOMAIN/launcher/" <<'PY'
import json, re, sys
path, base = sys.argv[1], sys.argv[2]
data = json.load(open(path, encoding="utf-8"))
for plat in data.get("platforms", {}).values():
    plat["url"] = base + re.sub(r".*/", "", plat["url"])
json.dump(data, open(path, "w", encoding="utf-8"), indent=2, ensure_ascii=False)
PY
    echo "$TAG" > "$TMP/.versao"
    docker compose exec -T api mkdir -p /data/launcher
    # Instaladores primeiro, latest.json e .versao por último: ninguém recebe um
    # link para um arquivo que ainda não chegou.
    for f in "$TMP"/*.exe* "$TMP"/*.msi*; do
      if [ -e "$f" ]; then docker compose cp "$f" api:/data/launcher/; fi
    done
    docker compose cp "$TMP/latest.json" api:/data/launcher/latest.json
    docker compose cp "$TMP/.versao" api:/data/launcher/.versao
    echo "   publicado $TAG"
  fi
else
  echo "   nenhum release publicado ainda (ou o token não tem acesso) — launcher não mudou"
fi

passo "Conferindo a API"
for _ in $(seq 1 40); do
  if docker compose exec -T api curl -fsS http://localhost:8080/health >/dev/null 2>&1; then
    echo "   ok — https://$API_DOMAIN/health"
    exit 0
  fi
  sleep 3
done
docker compose logs --tail 50 api
erro "a API não respondeu ao /health"

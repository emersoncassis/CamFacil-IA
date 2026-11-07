#!/bin/sh
# CamFácil IA · instala o painel web numa VPS Linux (ou no seu PC com Docker).
#   curl -fsSL https://cfia.ysafe.com.br/instalar-painel.sh | sudo sh
# Pergunta o domínio, instala o Docker se faltar, gera a senha do banco e sobe tudo
# com HTTPS automático. Rodar de novo atualiza para a versão mais nova.
set -eu
DIR="${CAMFACIL_PAINEL_DIR:-/opt/camfacil-painel}"
URL="https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/deploy/docker-compose.vps.yml"

if ! command -v docker >/dev/null 2>&1; then
  echo "Instalando o Docker…"
  curl -fsSL https://get.docker.com | sh
fi

mkdir -p "$DIR"
cd "$DIR"
curl -fsSL "$URL" -o docker-compose.yml

if [ ! -f .env ]; then
  DOMINIO="${DOMINIO:-}"
  if [ -z "$DOMINIO" ] && [ -r /dev/tty ]; then
    printf "Domínio do painel (ex.: painel.suaempresa.com.br, ou localhost para testar): " > /dev/tty
    read -r DOMINIO < /dev/tty
  fi
  [ -n "$DOMINIO" ] || { echo "Informe o domínio: DOMINIO=painel.seusite.com.br sh instalar-painel.sh"; exit 1; }
  umask 077
  cat > .env <<ENV
DOMINIO=$DOMINIO
POSTGRES_PASSWORD=$(tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40)
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
ENV
  echo "Configuração criada em $DIR/.env"
fi

docker compose pull
docker compose up -d
DOMINIO=$(grep '^DOMINIO=' .env | cut -d= -f2)
echo
echo "Painel subindo em https://$DOMINIO"
echo "1. Aponte o DNS do domínio (registro A) para o IP desta máquina."
echo "2. Coloque GOOGLE_CLIENT_ID e GOOGLE_CLIENT_SECRET em $DIR/.env (passo a passo no README)."
echo "3. Rode de novo: cd $DIR && docker compose up -d"

#!/usr/bin/env sh
# Instala ou atualiza o site do CamFácil IA no servidor do Dokploy.
# Rodar como root no servidor. Pode rodar de novo para atualizar.
set -eu
DIR=/opt/camfacil
REPO=https://github.com/emersoncassis/CamFacil-IA.git

command -v docker >/dev/null || { echo "Docker não encontrado."; exit 1; }
docker network inspect dokploy-network >/dev/null 2>&1 || { echo "Rede dokploy-network não encontrada (Dokploy instalado?)."; exit 1; }

if [ -d "$DIR/.git" ]; then git -C "$DIR" pull --ff-only; else git clone --depth 1 "$REPO" "$DIR"; fi
cd "$DIR/deploy"

if [ ! -f .env ]; then
  umask 077
  cat > .env <<ENV
POSTGRES_PASSWORD=$(tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40)
CFIA_DOMINIO=cfia.ysafe.com.br
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
ENV
  echo "Criado $DIR/deploy/.env (senha do banco gerada)."
fi

docker compose -f docker-compose.servidor.yml --env-file .env up -d --build
echo "Aguardando o site responder…"
for i in $(seq 1 60); do
  if docker exec cfia-api true 2>/dev/null && curl -fsS "https://$(grep ^CFIA_DOMINIO .env | cut -d= -f2)/api/saude" 2>/dev/null; then
    echo; echo "Pronto: https://$(grep ^CFIA_DOMINIO .env | cut -d= -f2)"; exit 0
  fi
  sleep 5
done
echo "O contêiner subiu, mas o HTTPS ainda não respondeu. Confira o DNS e veja: docker logs cfia-api"

#!/bin/sh
# CamFácil IA · instalador para Linux, Raspberry Pi e mini PC (x86_64 e ARM 64).
#   curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
# Baixa o programa, instala o ffmpeg se faltar e abre a dashboard.
set -eu
REPO="emersoncassis/CamFacil-IA"
case "$(uname -m)" in
  x86_64 | amd64) ARQ=linux-x86_64 ;;
  aarch64 | arm64) ARQ=linux-arm64 ;;
  *) echo "Este processador ($(uname -m)) ainda não tem versão pronta. Use um PC x86_64 ou um Raspberry Pi com sistema 64 bits."; exit 1 ;;
esac
DEST="${CAMFACIL_DIR:-$HOME/.local/bin}"
mkdir -p "$DEST"
echo "Baixando o CamFácil IA ($ARQ)…"
curl -fSL --progress-bar "https://github.com/$REPO/releases/latest/download/camfacil-$ARQ" -o "$DEST/camfacil.novo"
chmod +x "$DEST/camfacil.novo"
mv -f "$DEST/camfacil.novo" "$DEST/camfacil"

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "Instalando o ffmpeg (mostra o vídeo das câmeras). Pode pedir a sua senha."
  if command -v apt-get >/dev/null 2>&1; then sudo apt-get install -y ffmpeg || true
  elif command -v dnf >/dev/null 2>&1; then sudo dnf install -y ffmpeg || true
  elif command -v pacman >/dev/null 2>&1; then sudo pacman -S --noconfirm ffmpeg || true
  else echo "Instale o ffmpeg pelo gerenciador de pacotes do seu sistema."; fi
fi

case ":$PATH:" in *":$DEST:"*) ;; *) echo "Dica: para abrir depois, rode $DEST/camfacil" ;; esac
"$DEST/camfacil" --autoiniciar || true
echo "Pronto. Abrindo o CamFácil IA no navegador…"
exec "$DEST/camfacil"

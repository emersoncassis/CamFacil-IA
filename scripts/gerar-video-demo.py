#!/usr/bin/env python3
"""Gera o vídeo da câmera de demonstração do programa a partir da mesma cena
da Formação de IA para CFTV (site): a foto da loja e as pessoas que entram
pela porta, vêm até a câmera, param no balcão e saem.

Os trajetos copiam os de crates/cfia-api/web/escola.js (PORTA, PERTO, BALCAO),
para o técnico treinar no programa com a mesma imagem que viu no curso.

Uso:  python3 scripts/gerar-video-demo.py
Saída: crates/camfacil/assets/demo-loja.mp4 (embutido no binário)
Precisa de: Pillow e ffmpeg no PATH.
"""
import subprocess
from pathlib import Path

from PIL import Image

RAIZ = Path(__file__).resolve().parent.parent
IMG = RAIZ / "crates/cfia-api/web/img"
SAIDA = RAIZ / "crates/camfacil/assets/demo-loja.mp4"

W, H = 960, 540  # coordenadas da cena do site
OUT_W, OUT_H = 640, 360  # o programa trabalha em 640 de largura
FPS = 10
SPRITE_W, SPRITE_H = 126, 250

PORTA = (326, 165, 0.3)
PERTO = (292, 350, 1.4)
BALCAO = (442, 245, 1.0)


def lerp(a, b, k):
    return a + (b - a) * k


def trajeto(k, de, para, dx=0):
    return (lerp(de[0], para[0], k) + dx, lerp(de[1], para[1], k), lerp(de[2], para[2], k))


def ease_out(k):
    return 1 - (1 - k) ** 3


# Roteiro (segundos). Começa e termina com a loja vazia para o laço não "pular".
# Mesmas passagens dos exercícios do site: duas pessoas entram, uma sai, e um
# cliente entra, para na frente do balcão e depois vai embora.
ROTEIRO = [
    # (início, duração, de, para, dx, lado, suavizar)
    (2.0, 4.2, PORTA, PERTO, 0, "frente", False),
    (8.0, 4.2, PORTA, PERTO, -55, "frente", False),
    (14.0, 4.2, PERTO, PORTA, 40, "costas", False),
    (20.0, 2.6, PORTA, BALCAO, 0, "frente", True),
    (22.6, 5.0, BALCAO, BALCAO, 0, "frente", False),  # parado sendo atendido
    (27.6, 3.4, BALCAO, PORTA, 0, "costas", False),
]
DURACAO = 33.0


def main():
    fundo = Image.open(IMG / "loja.webp").convert("RGBA")
    sprites = {
        lado: Image.open(IMG / f"pessoa-{lado}.webp").convert("RGBA")
        for lado in ("frente", "costas")
    }
    cache = {}

    def sprite(lado, s):
        chave = (lado, round(s, 3))
        if chave not in cache:
            w = max(1, round(SPRITE_W * s))
            h = max(1, round(SPRITE_H * s))
            cache[chave] = sprites[lado].resize((w, h), Image.LANCZOS)
        return cache[chave]

    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    ff = subprocess.Popen(
        [
            "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
            "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{OUT_W}x{OUT_H}",
            "-r", str(FPS), "-i", "pipe:0",
            "-c:v", "libx264", "-preset", "veryslow", "-crf", "30",
            "-tune", "stillimage", "-g", str(FPS * 4),
            "-pix_fmt", "yuv420p", "-movflags", "+faststart",
            str(SAIDA),
        ],
        stdin=subprocess.PIPE,
    )
    total = int(DURACAO * FPS)
    for n in range(total):
        t = n / FPS
        quadro = fundo.copy()
        for ini, dur, de, para, dx, lado, suave in ROTEIRO:
            if ini <= t < ini + dur:
                k = (t - ini) / dur
                if suave:
                    k = ease_out(k)
                x, y, s = trajeto(k, de, para, dx)
                spr = sprite(lado, s)
                quadro.alpha_composite(spr, (round(x), round(y)))
        rgb = quadro.convert("RGB").resize((OUT_W, OUT_H), Image.LANCZOS)
        ff.stdin.write(rgb.tobytes())
    ff.stdin.close()
    if ff.wait() != 0:
        raise SystemExit("ffmpeg falhou")
    print(f"{SAIDA} ({SAIDA.stat().st_size // 1024} KB, {DURACAO:.0f} s)")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Gera o letreiro animado dos apoiadores para o README.

Lê site-docs/assets/mural.json (a mesma fonte do mural do site) e escreve
docs/assets/mural-apoiadores.svg: uma faixa preta onde os apoiadores passam
da direita para a esquerda, como eventos que a IA detectou.

Uso: python3 scripts/gerar-mural-svg.py   (rode de novo sempre que o mural mudar)
"""
import json
from html import escape
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
ORIGEM = RAIZ / "site-docs/assets/mural.json"
DESTINO = RAIZ / "docs/assets/mural-apoiadores.svg"

LARGURA, ALTURA = 900, 64
ROTULO = 168          # largura da etiqueta fixa à esquerda
VELOCIDADE = 45       # pixels por segundo
LARG_NOME, LARG_DESC = 8.6, 7.0   # largura média aproximada por caractere


def descricao(a):
    partes = [a.get("nivel") or "Apoiador"]
    if a.get("criou"):
        partes.append("criou o " + a["criou"])
    if a.get("fez"):
        partes.append(a["fez"])
    if a.get("pediu"):
        partes.append("pediu: " + a["pediu"])
    texto = " · ".join(partes)
    if a.get("recado"):
        texto += f" · “{a['recado']}”"
    return texto


def main():
    dados = json.loads(ORIGEM.read_text(encoding="utf-8"))
    pessoas = [a for a in dados.get("apoiadores", []) if a.get("nome")]

    itens, x = [], 0.0
    for a in pessoas:
        nome = a["nome"] + (f" · {a['empresa']}" if a.get("empresa") else "")
        desc = descricao(a)
        w_nome = len(nome) * LARG_NOME
        w_desc = len(desc) * LARG_DESC
        itens.append(
            f'<g transform="translate({x:.0f} 0)">'
            f'<circle cx="6" cy="32" r="4" fill="#06cb3f"/>'
            f'<text x="20" y="37" class="n">{escape(nome)}</text>'
            f'<text x="{20 + w_nome + 12:.0f}" y="37" class="d">{escape(desc)}</text>'
            f"</g>"
        )
        x += 20 + w_nome + 12 + w_desc + 56
    ciclo = max(x, LARGURA - ROTULO)
    faixa = "".join(itens)
    duracao = ciclo / VELOCIDADE

    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="{LARGURA}" height="{ALTURA}" viewBox="0 0 {LARGURA} {ALTURA}" role="img" aria-label="Apoiadores do CamFácil IA: {escape(', '.join(a['nome'] for a in pessoas))}">
  <style>
    .n {{ font: 600 15px system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; fill: #ededed; }}
    .d {{ font: 400 13px system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; fill: #b4b4b4; }}
    .r {{ font: 500 11px ui-monospace, 'Cascadia Mono', Consolas, monospace; fill: #06cb3f; letter-spacing: .08em; }}
  </style>
  <defs>
    <clipPath id="palco"><rect x="{ROTULO}" y="1" width="{LARGURA - ROTULO - 1}" height="{ALTURA - 2}"/></clipPath>
    <linearGradient id="some" x1="0" x2="1">
      <stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#000" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="varre" x1="0" x2="1">
      <stop offset="0" stop-color="#06cb3f" stop-opacity="0"/><stop offset=".5" stop-color="#06cb3f" stop-opacity=".16"/><stop offset="1" stop-color="#06cb3f" stop-opacity="0"/>
    </linearGradient>
  </defs>
  <rect x=".5" y=".5" width="{LARGURA - 1}" height="{ALTURA - 1}" rx="10" fill="#000" stroke="#232323"/>
  <g clip-path="url(#palco)">
    <g>
      <g transform="translate({ROTULO + 24} 0)">{faixa}<g transform="translate({ciclo:.0f} 0)">{faixa}</g></g>
      <animateTransform attributeName="transform" type="translate" from="0 0" to="{-ciclo:.0f} 0" dur="{duracao:.1f}s" repeatCount="indefinite"/>
    </g>
    <rect x="{ROTULO}" y="1" width="140" height="{ALTURA - 2}" fill="url(#varre)">
      <animate attributeName="x" values="{ROTULO - 140};{LARGURA}" dur="3.2s" repeatCount="indefinite"/>
    </rect>
    <rect x="{ROTULO}" y="1" width="36" height="{ALTURA - 2}" fill="url(#some)"/>
  </g>
  <g>
    <path d="M14 22v-8h8M{ROTULO - 22} 14h8v8M{ROTULO - 14} 42v8h-8M22 50h-8v-8" fill="none" stroke="#06cb3f" stroke-width="2"/>
    <circle cx="30" cy="32" r="4" fill="#06cb3f"><animate attributeName="opacity" values="1;.25;1" dur="1.2s" repeatCount="indefinite"/></circle>
    <text x="42" y="36" class="r">APOIADORES</text>
    <line x1="{ROTULO}" y1="12" x2="{ROTULO}" y2="{ALTURA - 12}" stroke="#232323"/>
  </g>
</svg>
"""
    DESTINO.write_text(svg, encoding="utf-8")
    print(f"{DESTINO.relative_to(RAIZ)}: {len(pessoas)} apoiadores, ciclo de {duracao:.0f} s")


if __name__ == "__main__":
    main()

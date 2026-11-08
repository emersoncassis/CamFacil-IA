#!/usr/bin/env python3
"""Confere a escolha de canais de gravador (DVR/NVR) nas telas (Playwright).

Uso: com o programa rodando e uma câmera cadastrada que veio de gravador
(campo `canal` preenchido):

    python3 scripts/telas-gravador.py http://localhost:7070 <id_camera_de_gravador>

As respostas de /api/conectar e /api/gravador/adicionar são simuladas (16
canais, alguns com nome do ONVIF e alguns mudos). Nos 5 tamanhos confere:
título com o número de canais, grade paginada sem rolagem, escolher vários
canais, substream por padrão, pedido enviado certo, campo Canal no Editar
câmera e nenhum erro de console.
"""
import json
import sys
from playwright.sync_api import sync_playwright

TAMANHOS = [(360, 640), (390, 844), (768, 1024), (1366, 768), (1920, 1080)]

CANAIS = [
    {"canal": n, "nome": {1: "Portão", 2: "Garagem", 5: "Fundos"}.get(n), "responde": n not in (7, 8, 15, 16)}
    for n in range(1, 17)
]
CONECTAR = {"tipo": "gravador", "token": "t0k3n", "nome": "NVR Loja", "fabricante": "intelbras",
            "endereco": "rtsp://admin:***@192.0.2.60:554/cam/realmonitor?channel=1&subtype=1",
            "origem": "onvif", "canais": CANAIS}


def sem_rolagem(p):
    return p.evaluate("document.documentElement.scrollHeight <= innerHeight && document.documentElement.scrollWidth <= innerWidth")


def alcancavel(p, sel):
    """O botão está na tela e nada o cobre (nem a barra de abas, nem o corte do painel)."""
    return p.evaluate("""(s) => { const e = document.querySelector(s); if (!e) return false;
        const r = e.getBoundingClientRect(); if (!r.width || r.bottom > innerHeight || r.top < 0) return false;
        const x = r.left + r.width / 2, y = r.top + r.height / 2; const a = document.elementFromPoint(x, y);
        if (!a || !(a === e || e.contains(a))) return false;
        for (let p = e.parentElement; p; p = p.parentElement) { const q = p.getBoundingClientRect();
          const cs = getComputedStyle(p); if (cs.overflow !== 'visible' && (y > q.bottom || y < q.top)) return false; }
        return true; }""", sel)


def main():
    base, cam_gravador = sys.argv[1], int(sys.argv[2])
    falhas = []
    with sync_playwright() as pw:
        nav = pw.chromium.launch(channel="chrome", args=["--no-sandbox"])
        for w, h in TAMANHOS:
            ctx = nav.new_context(viewport={"width": w, "height": h}, has_touch=w < 800)
            p = ctx.new_page()
            erros, pedidos = [], []
            p.on("console", lambda m: erros.append(m.text) if m.type == "error" else None)
            p.on("pageerror", lambda e: erros.append(str(e)))
            p.route("**/api/conectar", lambda r: r.fulfill(status=200, content_type="application/json", body=json.dumps(CONECTAR)))

            def adicionar(r):
                pedidos.append(json.loads(r.request.post_data))
                r.fulfill(status=200, content_type="application/json",
                          body=json.dumps({"cameras": [{"id": 999, "nome": "x"}], "auto_ip": False, "mac_conhecido": True}))
            p.route("**/api/gravador/adicionar", adicionar)
            tam = f"{w}x{h}"

            def ok(cond, msg):
                if not cond:
                    falhas.append(f"{tam}: {msg}")

            p.goto(base + "/")
            p.wait_for_function("typeof gravadorMostrar === 'function'")
            p.wait_for_timeout(2500)
            p.evaluate("showTab('cameras')")
            p.wait_for_timeout(1500)
            p.evaluate("irSub('cameras', 'ligar', false); ligarEscolher('192.0.2.60', 'Intelbras', null, 'NVD 1416')")
            p.wait_for_timeout(200)
            p.fill("#ligar-senha", "s3nh@")
            p.click("#ligar-conectar")
            p.wait_for_selector("#ligar-gravador:not([hidden])")
            p.wait_for_timeout(400)
            titulo = p.text_content("#ligar-gravador-titulo")
            ok("16 canais" in titulo, f"título errado: {titulo}")
            ok(p.input_value("#ligar-gravador-stream") == "secundario", "não começou no substream")
            visiveis = p.locator("#ligar-gravador-canais > li:not(.pg-fora)")
            n = visiveis.count()
            pager = p.locator("#ligar-gravador-canais + .pager:not([hidden])").count()
            ok(n == 16 and pager == 0 or 0 < n < 16 and pager == 1, f"paginação errada ({n} visíveis, pager {pager})")
            ok(sem_rolagem(p), "escolha de canais rolou a página")
            ok(alcancavel(p, "#ligar-gravador-add"), "botão Adicionar câmeras fora da tela ou coberto")
            # Escolhe o canal 2 na primeira página e o 9 na seguinte.
            p.locator("#ligar-gravador-canais input[value='2']").check()
            ok("Garagem" in p.text_content("#ligar-gravador-canais"), "nome do ONVIF não aparece")
            while p.locator("#ligar-gravador-canais input[value='9']").is_hidden():
                p.click("#ligar-gravador-canais + .pager [data-d='1']")
            p.locator("#ligar-gravador-canais input[value='9']").check()
            ok(sem_rolagem(p), "página 2 dos canais rolou")
            ok("3 escolhido" in p.text_content("#ligar-gravador-conta"), "contagem errada")
            p.select_option("#ligar-gravador-stream", "principal")
            p.screenshot(path=f"gravador-{tam}.png")
            p.click("#ligar-gravador-add")
            p.wait_for_selector("#ligar-ok:not([hidden])")
            ok(pedidos and [c["canal"] for c in pedidos[0]["canais"]] == [1, 2, 9], f"canais enviados errados: {pedidos}")
            ok(pedidos and pedidos[0]["stream"] == "principal" and pedidos[0]["token"] == "t0k3n", "stream/token errados")
            ok(pedidos and pedidos[0]["nome"] == "NVR Loja", "nome do gravador não foi")
            ok(sem_rolagem(p), "tela de sucesso rolou")

            # Editar câmera de gravador: campo Canal aparece e cabe.
            p.unroute("**/api/gravador/adicionar")
            p.evaluate("carregarCameras()")
            p.wait_for_timeout(600)
            p.evaluate(f"irSub('cameras', 'suas', false); editarCamera({cam_gravador})")
            p.wait_for_selector("#cam-dlg[open]")
            ok(p.locator("#cam-canal-campo").is_visible(), "campo Canal não aparece no Editar")
            dlg = p.evaluate("(() => { const d = document.getElementById('cam-dlg'); const r = d.getBoundingClientRect(); return [r.top, r.bottom, d.scrollHeight, d.clientHeight]; })()")
            ok(dlg[0] >= 0 and dlg[1] <= h and dlg[2] <= dlg[3] + 1, f"Editar (gravador) não cabe {dlg}")
            ok(sem_rolagem(p), "Editar (gravador) rolou a página")
            p.screenshot(path=f"gravador-editar-{tam}.png")
            p.click("#cam-dlg-cancelar")
            # Erros esperados: o vídeo da câmera 999 simulada não existe.
            erros = [e for e in erros if "404" not in e]
            ok(not erros, f"erros de console: {erros}")
            ctx.close()
        nav.close()
    if falhas:
        print("FALHOU:\n  " + "\n  ".join(falhas))
        sys.exit(1)
    print(f"ok: {len(TAMANHOS)} tamanhos, escolha de canais e Editar sem rolagem e sem erro de console")


if __name__ == "__main__":
    main()

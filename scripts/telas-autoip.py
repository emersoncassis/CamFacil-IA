#!/usr/bin/env python3
"""Confere a chave de autocorreção de IP nas telas (Playwright, fora do binário).

Uso: com o programa rodando (`cargo run -p camfacil -- --no-browser`) e pelo
menos uma câmera RTSP com MAC aprendido, uma RTSP sem MAC e uma demo:

    python3 scripts/telas-autoip.py http://localhost:7070 <id_com_mac> <id_sem_mac> <id_demo>

Nos 5 tamanhos do DESIGN.md confere, no Ligar câmera e no Editar câmera:
chave visível e clicável pelo rótulo, foco visível, travada sem MAC, escondida
para demo, nenhuma rolagem e nenhum erro de console.
"""
import sys
from playwright.sync_api import sync_playwright

TAMANHOS = [(360, 640), (390, 844), (768, 1024), (1366, 768), (1920, 1080)]


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
    base, com_mac, sem_mac, demo = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
    falhas = []
    with sync_playwright() as pw:
        nav = pw.chromium.launch(channel="chrome", args=["--no-sandbox"])
        for w, h in TAMANHOS:
            ctx = nav.new_context(viewport={"width": w, "height": h}, has_touch=w < 800)
            p = ctx.new_page()
            erros = []
            p.on("console", lambda m: erros.append(m.text) if m.type == "error" else None)
            p.on("pageerror", lambda e: erros.append(str(e)))
            p.goto(base + "/")
            p.wait_for_function("typeof ligarEscolher === 'function' && document.querySelector('[data-i18n=\"autoip.rotulo\"]').textContent.length > 0")
            tam = f"{w}x{h}"

            def ok(cond, msg):
                if not cond:
                    falhas.append(f"{tam}: {msg}")

            # ---- Ligar câmera (adicionar) ----
            p.wait_for_timeout(2500)  # deixa o início da tela terminar (ele escolhe a aba)
            p.evaluate("showTab('cameras')")
            p.wait_for_timeout(1500)  # e a busca automática de câmeras
            p.evaluate("irSub('cameras', 'ligar', false); ligarEscolher('192.0.2.50', 'Intelbras', null)")
            p.wait_for_timeout(300)
            sw = p.locator("#ligar-autoip")
            ok(sw.is_visible(), "chave do Ligar câmera não aparece")
            chk = p.locator("#ligar-autoip input")
            ok(not chk.is_checked(), "chave do Ligar câmera não começa desligada")
            p.locator("#ligar-autoip .autoip-txt > span").first.click()
            ok(chk.is_checked(), "clicar no rótulo não liga a chave")
            p.locator("#ligar-autoip .autoip-txt > span").first.click()
            # Foco visível pelo teclado.
            p.focus("#ligar-form input[name=nome]")
            p.keyboard.press("Tab")
            foco = p.evaluate("""(() => { const a = document.activeElement; if (!a || a.name !== 'auto_ip') return null;
                return getComputedStyle(a.nextElementSibling).outlineStyle; })()""")
            ok(foco == "solid", f"foco não visível na chave do Ligar câmera ({foco})")
            ok(sem_rolagem(p), "Ligar câmera rolou a página")
            ok(alcancavel(p, "#ligar-conectar"), "botão Conectar fora da tela ou coberto")
            r = sw.bounding_box()
            ok(r and r["y"] + r["height"] <= h, "chave do Ligar câmera fora da tela")
            ok(r and r["height"] >= 24, "alvo de toque pequeno")
            p.screenshot(path=f"autoip-ligar-{tam}.png")

            # ---- Editar câmera ----
            p.evaluate("carregarCameras()")
            p.wait_for_timeout(500)
            p.evaluate(f"irSub('cameras', 'suas', false); editarCamera({com_mac})")
            p.wait_for_selector("#cam-dlg[open]")
            ce = p.locator("#cam-form input[name=auto_ip]")
            ok(p.locator("#cam-autoip-campo").is_visible(), "chave do Editar não aparece (RTSP com MAC)")
            ok(ce.is_enabled(), "chave do Editar travada com MAC conhecido")
            antes = ce.is_checked()
            p.locator("#cam-autoip-campo .autoip-txt > span").first.click()
            ok(ce.is_checked() != antes, "clicar no rótulo do Editar não muda a chave")
            dlg = p.evaluate("(() => { const d = document.getElementById('cam-dlg'); const r = d.getBoundingClientRect(); return [r.top, r.bottom, d.scrollHeight, d.clientHeight]; })()")
            ok(dlg[0] >= 0 and dlg[1] <= h and dlg[2] <= dlg[3] + 1, f"diálogo Editar não cabe/rola {dlg}")
            ok(sem_rolagem(p), "Editar câmera rolou a página")
            ok(alcancavel(p, "#cam-dlg-salvar"), "botão Salvar fora da tela ou coberto")
            p.screenshot(path=f"autoip-editar-{tam}.png")
            p.click("#cam-dlg-cancelar")

            p.evaluate(f"editarCamera({sem_mac})")
            p.wait_for_selector("#cam-dlg[open]")
            ok(ce.is_disabled(), "chave não travou sem MAC")
            ajuda = p.text_content("#cam-autoip-ajuda")
            ok("Aguardando a primeira varredura" in ajuda, f"dica sem MAC errada: {ajuda}")
            ok(sem_rolagem(p), "Editar (sem MAC) rolou a página")
            p.click("#cam-dlg-cancelar")

            p.evaluate(f"editarCamera({demo})")
            p.wait_for_selector("#cam-dlg[open]")
            ok(not p.locator("#cam-autoip-campo").is_visible(), "chave aparece para câmera demo")
            p.click("#cam-dlg-cancelar")

            ok(not erros, f"erros de console: {erros}")
            ctx.close()
        nav.close()
    if falhas:
        print("FALHOU:\n  " + "\n  ".join(falhas))
        sys.exit(1)
    print(f"ok: {len(TAMANHOS)} tamanhos, adicionar e editar sem rolagem e sem erro de console")


if __name__ == "__main__":
    main()

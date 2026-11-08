#!/usr/bin/env python3
"""Confere o MCP com HTTPS (Tailscale Serve/Funnel, ngrok) nas telas (Playwright).

Uso: com o programa rodando,

    python3 scripts/telas-mcp-https.py http://localhost:7070

As respostas de /api/mcp, /api/mcp/https e /api/ngrok são simuladas. Nos 5
tamanhos confere, sem rolar a página e sem erro de console:
- MCP ligado + Tailscale pronto: passo "Endereço HTTPS", Serve publica e mostra
  URL com token e cabeçalho; Funnel pede confirmação no diálogo da página;
- Tailscale sem HTTPS: passo a passo com o link do painel;
- Android: mensagem de que não há Serve/Funnel;
- Acesso de fora → ngrok: pop-up obrigatório antes de ligar ("Usar o
  Tailscale" não liga; "Entendi" liga), aviso amarelo com o tempo ligado,
  usuário e senha na tela.
"""
import json
import sys
from playwright.sync_api import sync_playwright

TAMANHOS = [(360, 640), (390, 844), (768, 1024), (1366, 768), (1920, 1080)]
TOKEN = "REMOVIDO-TOKEN-MCP"

MCP = {"ligado": True, "escrita": False, "url": "http://localhost:7070/mcp", "token": TOKEN,
       "exemplo": json.dumps({"mcpServers": {"camfacil": {"command": "/opt/camfacil/camfacil", "args": ["--mcp"]}}}, indent=2),
       "claude_code": "claude mcp add camfacil -- /opt/camfacil/camfacil --mcp"}
BASE_HTTPS = {"sistema": "linux", "android": False, "mcp_ligado": True, "instalado": True,
              "versao": "1.76.1", "versao_ok": True, "versao_minima": "1.52",
              "links": {"dns": "https://login.tailscale.com/admin/dns", "funnel": "https://login.tailscale.com/admin/acls/file"},
              "logado": True, "nome": "loja-pc.tail1234.ts.net", "https_ok": True, "funnel_ok": True,
              "funnel_portas": [443, 8443, 10000], "publicado": None,
              "comandos": {"serve": "tailscale serve --bg --https=443 --set-path=/mcp http://127.0.0.1:7070/mcp",
                           "funnel": "tailscale funnel --bg --https=443 --set-path=/mcp http://127.0.0.1:7070/mcp",
                           "desligar": None}}
NGROK = {"disponivel": True, "sistema": "linux", "instalado": False, "baixavel": True, "tem_authtoken": True,
         "auto24": False, "link_authtoken": "https://dashboard.ngrok.com/get-started/your-authtoken",
         "ligado": False, "erro": None}


def sem_rolagem(p):
    return p.evaluate("document.documentElement.scrollHeight <= innerHeight && document.documentElement.scrollWidth <= innerWidth")


def json_ok(r, corpo):
    r.fulfill(status=200, content_type="application/json", body=json.dumps(corpo))


def main():
    base = sys.argv[1]
    falhas = []
    with sync_playwright() as pw:
        nav = pw.chromium.launch(channel="chrome", args=["--no-sandbox"])
        for w, h in TAMANHOS:
            tam = f"{w}x{h}"

            def ok(cond, msg):
                if not cond:
                    falhas.append(f"{tam}: {msg}")

            ctx = nav.new_context(viewport={"width": w, "height": h}, has_touch=w < 800)
            ctx.grant_permissions(["clipboard-read", "clipboard-write"])
            p = ctx.new_page()
            erros, pedidos = [], []
            p.on("console", lambda m: erros.append(m.text) if m.type == "error" else None)
            p.on("pageerror", lambda e: erros.append(str(e)))
            estado = {"https": dict(BASE_HTTPS), "ngrok": dict(NGROK)}

            def rota_https(r):
                if r.request.method == "POST":
                    pedido = json.loads(r.request.post_data)
                    pedidos.append(("https", pedido))
                    s = dict(estado["https"])
                    if pedido["modo"] == "off":
                        s["publicado"] = None
                    else:
                        s["publicado"] = {"modo": pedido["modo"], "porta": 443, "url": "https://loja-pc.tail1234.ts.net/mcp"}
                        s["comandos"] = dict(s["comandos"], desligar=f"tailscale {pedido['modo']} --https=443 --set-path=/mcp off")
                    estado["https"] = s
                json_ok(r, estado["https"])

            def rota_ngrok(r):
                if r.request.method == "POST":
                    pedido = json.loads(r.request.post_data)
                    pedidos.append(("ngrok", pedido))
                    s = dict(estado["ngrok"])
                    if pedido.get("acao") == "ligar":
                        s.update(ligado=True, alvo=pedido["alvo"], url="https://ab12.ngrok-free.app", desde=0,
                                 ha_s=3 * 3600 + 120, usuario="camfacil" if pedido["alvo"] == "painel" else None,
                                 senha="9f8e7d6c5b4a39281706f5e4" if pedido["alvo"] == "painel" else None)
                        if pedido["alvo"] == "mcp":
                            s["url"] += "/mcp"
                    elif pedido.get("acao") == "desligar":
                        s = dict(NGROK)
                    estado["ngrok"] = s
                json_ok(r, estado["ngrok"])

            p.route("**/api/mcp", lambda r: json_ok(r, MCP))
            p.route("**/api/mcp/https", rota_https)
            p.route("**/api/ngrok", rota_ngrok)

            p.goto(base + "/")
            p.wait_for_function("typeof mcpHttpsRender === 'function' && typeof showTab === 'function'")
            p.wait_for_timeout(1500)
            p.evaluate("document.querySelectorAll('dialog[open]').forEach((d) => d.close())")
            p.evaluate("showTab('integracoes')")
            p.wait_for_timeout(500)
            p.evaluate("irSub('integracoes', 'mcp', false)")
            p.wait_for_timeout(800)

            # 1. Tailscale pronto: o passo HTTPS está aberto e oferece Serve e Funnel.
            ok(p.locator("#mcph-serve").is_visible(), "botão Tailscale Serve não aparece")
            ok(p.locator("#mcph-funnel").is_visible(), "botão Tailscale Funnel não aparece")
            ok("Escolha" in p.text_content("#mcph-status"), "estado 'pronto' errado")
            ok(sem_rolagem(p), "seção MCP rolou a página")
            p.screenshot(path=f"mcp-https-{tam}.png")

            # 2. Serve: publica e mostra URL com token e cabeçalho.
            p.click("#mcph-serve")
            p.wait_for_selector("#mcph-pronto:not([hidden])")
            ok(p.text_content("#mcph-url-token") == f"https://loja-pc.tail1234.ts.net/mcp?token={TOKEN}", "URL com token errada")
            ok(p.text_content("#mcph-cabecalho") == f"Authorization: Bearer {TOKEN}", "cabeçalho errado")
            ok(pedidos[-1] == ("https", {"modo": "serve", "confirmado": False}), f"pedido do Serve errado: {pedidos[-1:]}")
            ok(p.locator("#mcph-desligar").is_visible(), "sem botão para desligar")
            ok(sem_rolagem(p), "MCP publicado rolou a página")
            p.screenshot(path=f"mcp-https-serve-{tam}.png")

            # 3. Funnel: diálogo da página; cancelar não publica, confirmar publica.
            n = len(pedidos)
            p.click("#mcph-funnel")
            p.wait_for_selector("#confirma-dlg[open]")
            ok("publica o MCP na internet" in p.text_content("#confirma-texto"), "texto da confirmação do Funnel errado")
            ok(sem_rolagem(p), "confirmação do Funnel rolou")
            p.click("#confirma-nao")
            p.wait_for_timeout(200)
            ok(len(pedidos) == n, "Funnel publicou sem confirmar")
            p.click("#mcph-funnel")
            p.wait_for_selector("#confirma-dlg[open]")
            p.click("#confirma-sim")
            p.wait_for_function("document.querySelector('#mcph-status').textContent.includes('Funnel')")
            ok(pedidos[-1] == ("https", {"modo": "funnel", "confirmado": True}), f"pedido do Funnel errado: {pedidos[-1:]}")
            p.click("#mcph-desligar")
            p.wait_for_selector("#mcph-pronto[hidden]", state="attached")
            ok(pedidos[-1][1]["modo"] == "off", "desligar não pediu off")

            # 4. Claude Desktop / Claude Code: JSON e comando prontos para copiar.
            p.click("details.secao:has(#mcp-exemplo) > summary")
            p.wait_for_timeout(200)
            ok("--mcp" in p.text_content("#mcp-exemplo"), "JSON do Claude Desktop não aparece")
            ok(p.text_content("#mcp-claude-code").startswith("claude mcp add camfacil -- "), "comando do Claude Code errado")
            ok(sem_rolagem(p), "seção Claude Desktop rolou a página")

            # 5. Tailscale sem HTTPS: passo a passo com o link do painel.
            estado["https"] = dict(BASE_HTTPS, https_ok=False, funnel_ok=False)
            p.evaluate("mcphCarregar()")
            p.click("details.secao:has(#mcph-status) > summary")
            p.wait_for_selector("#mcph-passos:not([hidden])")
            ok("login.tailscale.com/admin/dns" in p.inner_html("#mcph-passos"), "sem link do painel do Tailscale")
            ok(p.locator("#mcph-serve").is_hidden(), "Serve aparece sem HTTPS")
            ok(sem_rolagem(p), "passo a passo rolou a página")

            # 6. Android: só a mensagem do app Tailscale.
            estado["https"] = {k: BASE_HTTPS[k] for k in ("sistema", "mcp_ligado", "links", "versao_minima")} | {"android": True, "instalado": False}
            estado["ngrok"] = dict(NGROK, disponivel=False)
            p.evaluate("ngrokCarregar().then(mcphCarregar)")
            p.wait_for_timeout(300)
            ok("Android" in p.text_content("#mcph-status"), "mensagem do Android não aparece")
            ok(p.locator("#mcph-ngrok").is_hidden() and p.locator("#mcph-serve").is_hidden(), "botões aparecem no Android")
            ok(sem_rolagem(p), "Android rolou a página")
            estado["https"] = dict(BASE_HTTPS)
            estado["ngrok"] = dict(NGROK)

            # 7. Acesso de fora → ngrok: pop-up obrigatório.
            p.evaluate("irSub('integracoes', 'tailscale', false); ngrokCarregar()")
            p.wait_for_timeout(300)
            p.click("details.secao:has(#ngrok-ligar) > summary")
            p.wait_for_timeout(200)
            ok(sem_rolagem(p), "seção ngrok rolou a página")
            n = len(pedidos)
            p.click("#ngrok-ligar")
            p.wait_for_selector("#ngrok-dlg[open]")
            texto = p.text_content("#ngrok-dlg-texto")
            ok("temporário" in texto and "Tailscale" in texto and "exposto" in texto, "texto do pop-up errado")
            ok(p.text_content("#ngrok-dlg-ligar") == "Entendi, ligar mesmo assim", "botão de ligar do pop-up errado")
            caixa = p.evaluate("(() => { const r = document.getElementById('ngrok-dlg').getBoundingClientRect(); return [r.top, r.bottom]; })()")
            ok(caixa[0] >= 0 and caixa[1] <= h, f"pop-up não cabe na tela {caixa}")
            ok(sem_rolagem(p), "pop-up rolou a página")
            p.screenshot(path=f"ngrok-popup-{tam}.png")
            p.click("#ngrok-dlg-tailscale")
            p.wait_for_timeout(200)
            ok(len(pedidos) == n, "'Usar o Tailscale' ligou o ngrok")
            p.click("#ngrok-ligar")
            p.wait_for_selector("#ngrok-dlg[open]")
            p.click("#ngrok-dlg-ligar")
            p.wait_for_selector("#ngrok-aviso:not([hidden])")
            ok(pedidos[-1] == ("ngrok", {"acao": "ligar", "alvo": "painel", "entendi": True}), f"pedido do ngrok errado: {pedidos[-1:]}")
            ok("3 h 2 min" in p.text_content("#ngrok-aviso"), "aviso amarelo sem o tempo ligado")
            ok(p.text_content("#ngrok-senha") == "9f8e7d6c5b4a39281706f5e4", "senha do painel não aparece")
            ok(p.locator("#ngrok-desligar").is_visible(), "sem botão Desligar")
            ok(sem_rolagem(p), "ngrok ligado rolou a página")
            p.screenshot(path=f"ngrok-ligado-{tam}.png")
            p.click("#ngrok-desligar")
            p.wait_for_selector("#ngrok-aviso[hidden]", state="attached")

            ok(not erros, f"erros de console: {erros}")
            ctx.close()
        nav.close()
    if falhas:
        print("FALHOU:\n  " + "\n  ".join(falhas))
        sys.exit(1)
    print(f"ok: {len(TAMANHOS)} tamanhos, MCP HTTPS e ngrok sem rolagem e sem erro de console")


if __name__ == "__main__":
    main()

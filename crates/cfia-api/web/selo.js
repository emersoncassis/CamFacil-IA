// Selo da Formação de IA para CFTV: cartão, imagem para stories (1080×1920) e compartilhar.
"use strict";
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const el = (tag, attrs = {}, ...kids) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) { if (k === "class") e.className = v; else if (k.startsWith("on")) e[k] = v; else e.setAttribute(k, v); }
    e.append(...kids.filter((k) => k != null));
    return e;
  };
  const dataBr = (iso) => { const [a, m, d] = (iso || "").split("-"); return d ? `${d}/${m}/${a}` : iso; };
  const urlSelo = (s) => `${location.origin}/selo/${encodeURIComponent(s.codigo)}`;
  const textoCompart = (s) => `Concluí a Formação de IA para CFTV do CamFácil IA: aprendi na prática a configurar IA de vídeo (linha de passagem, zona, alertas e saúde da câmera). Curso gratuito, no navegador: ${urlSelo(s)}`;

  // Desenha a arte do selo num canvas. Formato stories por padrão.
  function arteSelo(s, larguraReal = 1080, alturaReal = 1920) {
    // Desenha sempre em 1080×1920 e escala para o tamanho pedido.
    const largura = 1080, altura = 1920;
    const c = document.createElement("canvas"); c.width = larguraReal; c.height = alturaReal;
    const g = c.getContext("2d");
    g.scale(larguraReal / largura, alturaReal / altura);
    const verde = "#06cb3f";
    g.fillStyle = "#000"; g.fillRect(0, 0, largura, altura);
    // brilho
    const rad = g.createRadialGradient(largura / 2, altura * 0.42, 40, largura / 2, altura * 0.42, largura * 0.75);
    rad.addColorStop(0, "rgba(6,203,63,.22)"); rad.addColorStop(1, "rgba(6,203,63,0)");
    g.fillStyle = rad; g.fillRect(0, 0, largura, altura);
    // cantos de visor
    g.strokeStyle = verde; g.lineWidth = 8; g.lineCap = "round";
    const m = 70, L = 110;
    for (const [x, y, sx, sy] of [[m, m, 1, 1], [largura - m, m, -1, 1], [m, altura - m, 1, -1], [largura - m, altura - m, -1, -1]]) {
      g.beginPath(); g.moveTo(x, y + sy * L); g.lineTo(x, y); g.lineTo(x + sx * L, y); g.stroke();
    }
    const centro = (txt, y, fonte, cor = "#fff") => { g.fillStyle = cor; g.font = fonte; g.textAlign = "center"; g.fillText(txt, largura / 2, y); };
    centro("CAMFÁCIL IA", 280, "600 40px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", "rgba(255,255,255,.7)");
    centro("FORMAÇÃO DE IA PARA CFTV", 345, "700 46px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", verde);
    // medalha
    const cx = largura / 2, cy = 640, R = 190;
    g.save(); g.translate(cx, cy);
    g.beginPath();
    for (let i = 0; i < 24; i++) { const a = (i / 24) * Math.PI * 2; const r = i % 2 ? R : R * 0.9; g.lineTo(Math.cos(a) * r, Math.sin(a) * r); }
    g.closePath(); g.fillStyle = "#0a0a0a"; g.fill(); g.lineWidth = 10; g.strokeStyle = verde; g.stroke();
    g.beginPath(); g.arc(0, 0, R * 0.62, 0, Math.PI * 2); g.lineWidth = 4; g.strokeStyle = "rgba(6,203,63,.5)"; g.stroke();
    g.beginPath(); g.lineWidth = 22; g.lineCap = "round"; g.lineJoin = "round"; g.strokeStyle = verde;
    g.moveTo(-70, 5); g.lineTo(-20, 55); g.lineTo(80, -55); g.stroke();
    g.restore();
    centro("CONCLUÍDA", 930, "700 44px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", verde);
    // nome (quebra em 2 linhas se precisar)
    g.font = "700 72px system-ui, -apple-system, Segoe UI, Roboto, sans-serif";
    const palavras = s.nome.split(" "); const linhas = []; let atual = "";
    for (const p of palavras) { const t = atual ? `${atual} ${p}` : p; if (g.measureText(t).width > largura - 200 && atual) { linhas.push(atual); atual = p; } else atual = t; }
    if (atual) linhas.push(atual);
    linhas.slice(0, 2).forEach((l, i) => centro(l, 1070 + i * 84, "700 72px system-ui, -apple-system, Segoe UI, Roboto, sans-serif"));
    let y = 1070 + Math.min(linhas.length, 2) * 84 + 10;
    if (s.instagram) { centro(`@${s.instagram}`, y, "500 44px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", "rgba(255,255,255,.75)"); y += 70; }
    centro("Aprendeu na prática a configurar IA de vídeo:", y + 40, "400 36px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", "rgba(255,255,255,.75)");
    centro("linha de passagem · zona · alertas · saúde da câmera", y + 92, "400 34px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", "rgba(255,255,255,.6)");
    centro(dataBr(s.criado_em), y + 170, "500 36px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", "rgba(255,255,255,.6)");
    centro("cfia.ysafe.com.br/aprender", altura - 150, "600 40px ui-monospace, SFMono-Regular, Menlo, monospace", verde);
    centro("curso gratuito, no navegador · #sangueverde", altura - 100, "400 30px system-ui, -apple-system, Segoe UI, Roboto, sans-serif", "rgba(255,255,255,.55)");
    return c;
  }

  function cartaoSelo(s, { compacto = false } = {}) {
    const url = urlSelo(s);
    const img = el("img", { class: "selo-arte", alt: `Selo de ${s.nome}` });
    img.src = arteSelo(s, 540, 960).toDataURL("image/png");
    const baixar = el("a", { class: "btn primary", download: `selo-camfacil-ia-${s.codigo}.png` }, "Baixar imagem (stories)");
    baixar.href = arteSelo(s).toDataURL("image/png");
    const whats = el("a", { class: "btn", target: "_blank", rel: "noopener", href: `https://wa.me/?text=${encodeURIComponent(textoCompart(s))}` }, "WhatsApp");
    const linkedin = el("a", { class: "btn", target: "_blank", rel: "noopener", href: `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(url)}` }, "LinkedIn");
    const copiar = el("button", { class: "btn", type: "button" }, "Copiar texto e link");
    copiar.onclick = async () => { try { await navigator.clipboard.writeText(textoCompart(s)); copiar.textContent = "Copiado"; setTimeout(() => { copiar.textContent = "Copiar texto e link"; }, 1600); } catch (_) { /* sem permissão */ } };
    const partilhar = el("button", { class: "btn", type: "button" }, "Compartilhar…");
    if (navigator.share) partilhar.onclick = () => navigator.share({ title: "Formação de IA para CFTV · CamFácil IA", text: textoCompart(s), url }).catch(() => {}); else partilhar.hidden = true;
    return el("div", { class: `selo-cartao${compacto ? " compacto" : ""}` },
      img,
      el("div", { class: "selo-info" },
        el("p", { class: "kicker" }, "Selo verificado"),
        el("h2", {}, s.nome),
        el("p", { class: "hint" }, `Concluiu a Formação de IA para CFTV em ${dataBr(s.criado_em)}${s.instagram ? ` · @${s.instagram}` : ""}.`),
        el("div", { class: "selo-acoes" }, baixar, whats, linkedin, partilhar, copiar),
        el("p", { class: "tiny selo-url" }, el("a", { href: url }, url)),
        (() => {
          // Instrução do Instagram numa seção que abre: o cartão cabe na tela.
          const d = el("details", { class: "acord" }, el("summary", {}, el("span", {}, "Como postar no Instagram")),
            el("p", { class: "tiny" }, "Para o Instagram: baixe a imagem e poste nos stories. Se você autorizou e informou o seu Instagram, o selo será repostado pelo Emerson e pelo Papo Segurança, marcando você."));
          return d;
        })()));
  }

  async function paginaSelo() {
    const codigo = location.pathname.split("/selo/")[1] || "";
    const main = $("#selo-pagina");
    try {
      const r = await fetch(`/api/escola/selo/${encodeURIComponent(codigo)}`);
      if (!r.ok) throw new Error("Esse selo não existe ou o link está incompleto.");
      const { selo } = await r.json();
      document.title = `${selo.nome} · Selo Formação de IA para CFTV · CamFácil IA`;
      main.replaceChildren(el("div", { class: "palco" }, cartaoSelo(selo)),
        el("section", { class: "selo-chamada" },
          el("div", {}, el("h3", {}, "Quer o seu também?"),
            el("p", { class: "hint" }, "Seis lições, cinco minutos, no navegador. Você desenha a linha e a zona e a IA diz se acertou.")),
          el("a", { class: "btn primary", href: "/aprender" }, "Começar o curso")));
    } catch (e) {
      main.replaceChildren(el("p", { class: "msg err" }, e.message), el("a", { class: "btn", href: "/aprender" }, "Ir para o curso"));
    }
  }

  window.cartaoSelo = cartaoSelo;
  window.paginaSelo = paginaSelo;
  if (document.querySelector("#selo-pagina")) paginaSelo();
})();

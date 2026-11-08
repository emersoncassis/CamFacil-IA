// Páginas de texto do site sem rolagem: cada seção vira uma página com
// Anterior / Próxima e um índice que abre. Abas para painéis lado a lado
// (estúdio) quando a tela é estreita. JS puro, sem nada inline (CSP).
"use strict";
(() => {
  const el = (tag, attrs = {}, ...kids) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) { if (k === "class") e.className = v; else if (k.startsWith("on")) e[k] = v; else e.setAttribute(k, v); }
    e.append(...kids.filter((k) => k != null));
    return e;
  };
  const NS = "http://www.w3.org/2000/svg";
  const ico = (d) => { const s = document.createElementNS(NS, "svg"); s.setAttribute("class", "ico sm"); s.setAttribute("viewBox", "0 0 24 24"); const p = document.createElementNS(NS, "path"); p.setAttribute("d", d); s.append(p); return s; };

  // ---------- Páginas com avançar ----------
  // As seções são agrupadas em folhas do tamanho da tela: no monitor grande
  // cabem várias de uma vez; no celular, uma por folha. Refaz ao redimensionar.
  for (const caixa of document.querySelectorAll("[data-paginas]")) {
    const pags = [...caixa.querySelectorAll(":scope > .pag")];
    if (pags.length < 2) continue;
    const titulo = (p, i) => p.dataset.titulo || (p.querySelector("h2") || {}).textContent || `Parte ${i + 1}`;
    let folhas = [[0]];
    let atual = 0;
    const ant = el("button", { class: "btn", type: "button" }, ico("m15 5-7 7 7 7"), el("span", {}, "Anterior"));
    const prox = el("button", { class: "btn primary", type: "button" }, el("span", {}, "Próxima"), ico("m9 5 7 7-7 7"));
    const rot = el("span", { class: "rot" });
    const pos = el("span", { class: "mono" });
    const lista = el("ol", {});
    const indice = el("details", { class: "doc-indice" }, el("summary", { "aria-label": "Índice" }, pos, rot), lista);
    const nav = el("nav", { class: "doc-nav", "aria-label": "Partes do texto" }, ant, indice, prox);
    caixa.after(nav);
    const folhaDe = (i) => Math.max(0, folhas.findIndex((f) => f.includes(i)));
    const ir = (n, focar) => {
      atual = Math.max(0, Math.min(folhas.length - 1, n));
      const f = folhas[atual];
      pags.forEach((p, j) => p.classList.toggle("atual", f.includes(j)));
      pos.textContent = `${atual + 1} de ${folhas.length}`;
      rot.textContent = ` · ${titulo(pags[f[0]], f[0])}`;
      ant.hidden = atual === 0;
      prox.hidden = atual === folhas.length - 1;
      lista.querySelectorAll("button").forEach((b, j) => b.setAttribute("aria-current", String(f.includes(j))));
      indice.open = false;
      const alvo = pags[f[0]];
      history.replaceState(null, "", atual && alvo.id ? `#${alvo.id}` : location.pathname);
      if (focar) (alvo.querySelector("h1, h2") || alvo).focus({ preventScroll: true });
    };
    const agrupar = () => {
      const primeira = (folhas[atual] || [0])[0];
      caixa.classList.remove("paginado");
      nav.hidden = false; // o espaço da navegação conta
      const max = caixa.clientHeight;
      const alturas = pags.map((p) => p.offsetHeight + parseFloat(getComputedStyle(p).marginBottom || 0));
      folhas = [];
      let f = []; let soma = 0;
      alturas.forEach((h, i) => {
        if (f.length && soma + h > max) { folhas.push(f); f = []; soma = 0; }
        f.push(i); soma += h;
      });
      if (f.length) folhas.push(f);
      caixa.classList.add("paginado");
      nav.hidden = folhas.length < 2;
      ir(folhaDe(primeira));
    };
    pags.forEach((p, i) => {
      const h = p.querySelector("h1, h2"); if (h) h.tabIndex = -1;
      lista.append(el("li", {}, el("button", { type: "button", onclick: () => ir(folhaDe(i), true) }, titulo(p, i))));
    });
    // Narração (ex.: manifesto) pede para mostrar a seção que está falando.
    caixa.addEventListener("doc-ir", (ev) => ir(folhaDe(ev.detail)));
    ant.onclick = () => ir(atual - 1, true);
    prox.onclick = () => ir(atual + 1, true);
    addEventListener("keydown", (ev) => {
      if (ev.target.closest("input, textarea")) return;
      if (ev.key === "ArrowRight" || ev.key === "PageDown") ir(atual + 1);
      if (ev.key === "ArrowLeft" || ev.key === "PageUp") ir(atual - 1);
      if (ev.key === "Escape") indice.open = false;
    });
    addEventListener("click", (ev) => { if (!ev.target.closest(".doc-indice")) indice.open = false; });
    let t = 0;
    addEventListener("resize", () => { clearTimeout(t); t = setTimeout(agrupar, 120); });
    const h = location.hash.slice(1);
    folhas = [[0]];
    agrupar();
    const alvo = pags.findIndex((p) => p.id && p.id === h);
    if (alvo > 0) ir(folhaDe(alvo));
  }

  // ---------- Abas em tela estreita ----------
  for (const caixa of document.querySelectorAll("[data-abas]")) {
    const paineis = [...caixa.querySelectorAll(":scope > [data-aba]")];
    const mq = matchMedia(caixa.dataset.abas || "(max-width: 999px)");
    let atual = 0;
    const barra = el("div", { class: "abas-barra", role: "tablist" });
    const botoes = paineis.map((p, i) => el("button", { type: "button", role: "tab", onclick: () => escolher(i) }, p.dataset.aba));
    barra.append(...botoes);
    caixa.before(barra);
    function escolher(i) {
      atual = i;
      botoes.forEach((b, j) => b.setAttribute("aria-selected", String(j === i)));
      paineis.forEach((p, j) => p.classList.toggle("aba-atual", j === i));
    }
    const aplicar = () => { caixa.classList.toggle("com-abas", mq.matches); barra.hidden = !mq.matches; };
    mq.addEventListener("change", aplicar);
    escolher(Number(caixa.dataset.abaInicial || 0));
    aplicar();
  }
})();

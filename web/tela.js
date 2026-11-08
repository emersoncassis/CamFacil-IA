// CamFácil IA — tela única (zero rolagem, regra 9 do CLAUDE.md).
// Cada página vira: cabeçalho + sub-abas + painel. Só um assunto aparece por vez;
// o que não cabe vira seção que abre (acordeão) e listas longas paginam.
// JS puro, roda depois que app.js carregou as traduções (chamado por main()).
"use strict";

const tela = { paginas: {}, listas: new Set() };

function telaT(k) { return typeof t === "function" ? t(k) : k; }

// ---------- Sub-abas ----------
function grupos(page) {
  const vistos = [];
  page.querySelectorAll(".painel > [data-sub]").forEach((b) => {
    const g = b.dataset.sub;
    if (g !== "*" && !vistos.includes(g)) vistos.push(g);
  });
  return vistos;
}

function grupoDisponivel(page, g) {
  return [...page.querySelectorAll(`.painel > [data-sub="${g}"]`)].some((b) => !b.hidden);
}

function irSub(pageId, g, lembrar = true) {
  const page = document.getElementById(pageId);
  if (!page || !page.querySelector(".subnav")) return;
  const todos = grupos(page);
  const livres = todos.filter((x) => grupoDisponivel(page, x));
  if (!livres.includes(g)) g = livres[0] || todos[0];
  page.dataset.sub = g;
  page.querySelectorAll(".painel > [data-sub]").forEach((b) => {
    b.classList.toggle("fora", b.dataset.sub !== "*" && b.dataset.sub !== g);
  });
  page.querySelectorAll(".subtabs button").forEach((b) => {
    b.setAttribute("aria-selected", String(b.dataset.g === g));
    b.hidden = !livres.includes(b.dataset.g);
  });
  const sel = page.querySelector(".subsel");
  sel.value = g;
  [...sel.options].forEach((o) => { o.hidden = !livres.includes(o.value); });
  if (lembrar) { try { localStorage.setItem("cf-sub-" + pageId, g); } catch (_) { /* opcional */ } }
  paginarTudo(page);
  ajustarSubnav(page);
}

// Muitas sub-abas para a largura: vira um menu (select nativo, bom no toque).
function ajustarSubnav(page) {
  const nav = page.querySelector(".subnav");
  if (!nav || page.hidden) return;
  nav.classList.remove("compacto");
  const tabs = nav.querySelector(".subtabs");
  if (tabs.scrollWidth > tabs.clientWidth + 1) nav.classList.add("compacto");
}

function montarPagina(page) {
  const head = page.querySelector(":scope > .pagehead");
  const painel = document.createElement("div");
  painel.className = "painel";
  [...page.children].filter((c) => c !== head).forEach((c) => painel.append(c));
  page.append(painel);
  const gs = grupos(page);
  if (gs.length < 2) { page.classList.add("sem-subs"); return; }

  const nav = document.createElement("div");
  nav.className = "subnav";
  const tabs = document.createElement("div");
  tabs.className = "subtabs";
  tabs.setAttribute("role", "tablist");
  tabs.setAttribute("aria-label", telaT("sub.menu"));
  const sel = document.createElement("select");
  sel.className = "subsel";
  sel.setAttribute("aria-label", telaT("sub.menu"));
  for (const g of gs) {
    const rotulo = telaT(`sub.${page.id}.${g}`);
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "tab");
    b.dataset.g = g;
    b.textContent = rotulo;
    b.onclick = () => irSub(page.id, g);
    tabs.append(b);
    const o = document.createElement("option");
    o.value = g;
    o.textContent = rotulo;
    sel.append(o);
  }
  sel.onchange = () => irSub(page.id, sel.value);
  // Título do bloco igual ao da sub-aba: a sub-aba já diz, some o repetido.
  const norm = (x) => x.trim().toLowerCase();
  painel.querySelectorAll(":scope > [data-sub] > h2, :scope > [data-sub] > .block-head > h2, :scope > [data-sub] > .passo > h2").forEach((h) => {
    const g = h.closest("[data-sub]").dataset.sub;
    if (norm(h.textContent) === norm(telaT(`sub.${page.id}.${g}`))) h.classList.add("repete");
  });
  nav.append(tabs, sel);
  page.insertBefore(nav, painel);

  // Um bloco que aparece ou some (câmera aberta, passo 2…) muda as sub-abas.
  const obs = new MutationObserver(() => irSub(page.id, page.dataset.sub, false));
  painel.querySelectorAll(":scope > [data-sub]").forEach((b) => obs.observe(b, { attributes: true, attributeFilter: ["hidden"] }));

  let g = gs[0];
  try { g = localStorage.getItem("cf-sub-" + page.id) || g; } catch (_) { /* opcional */ }
  irSub(page.id, g, false);
}

// ---------- Acordeões: cada subtítulo (h3.sub) vira uma seção que abre ----------
let secoes = 0;
function montarSecoes(raiz) {
  raiz.querySelectorAll("h3.sub").forEach((h) => {
    const pai = h.parentElement;
    const nome = "sec-" + (pai.closest("[id]")?.id || "x");
    const d = document.createElement("details");
    d.className = "secao";
    d.setAttribute("name", nome);
    const s = document.createElement("summary");
    s.id = "secao-" + (++secoes);
    while (h.firstChild) s.append(h.firstChild);
    if (h.dataset.i18n) s.dataset.i18n = h.dataset.i18n;
    if ("aberta" in h.dataset) d.open = true;
    d.append(s);
    let n = h.nextSibling;
    while (n && !(n.nodeType === 1 && n.matches("h3.sub"))) {
      const prox = n.nextSibling;
      d.append(n);
      n = prox;
    }
    h.replaceWith(d);
  });
  // WebView antigo sem <details name>: fecha as irmãs na mão.
  raiz.addEventListener("toggle", (e) => {
    const d = e.target;
    if (!(d instanceof HTMLDetailsElement) || !d.open || !d.getAttribute("name")) return;
    document.querySelectorAll(`details[name="${d.getAttribute("name")}"]`).forEach((o) => { if (o !== d) o.open = false; });
  }, true);
}

// ---------- Paginação de listas e tabelas ----------
function porPagina(lista) {
  if (lista.dataset.porPagina) return Number(lista.dataset.porPagina);
  const h = window.innerHeight;
  return h < 700 ? 4 : h < 900 ? 6 : 8;
}

function paginar(lista) {
  const itens = [...lista.children].filter((c) => !c.classList.contains("empty") && !c.classList.contains("group"));
  const n = lista.cfN || porPagina(lista);
  const total = Math.max(1, Math.ceil(itens.length / n));
  let pag = Math.min(Number(lista.dataset.pag || 0), total - 1);
  lista.dataset.pag = String(pag);
  itens.forEach((c, i) => c.classList.toggle("pg-fora", Math.floor(i / n) !== pag));
  const ancora = lista.closest(".table-wrap") || lista;
  let pager = ancora.nextElementSibling;
  if (!pager || !pager.classList.contains("pager")) {
    pager = document.createElement("div");
    pager.className = "pager";
    const ant = document.createElement("button");
    ant.type = "button"; ant.className = "btn ghost sm"; ant.dataset.d = "-1";
    ant.setAttribute("aria-label", telaT("pag.anterior"));
    ant.innerHTML = '<svg class="ico sm" viewBox="0 0 24 24"><path d="m15 6-6 6 6 6"/></svg>';
    const txt = document.createElement("span");
    txt.className = "tiny mono";
    const prox = document.createElement("button");
    prox.type = "button"; prox.className = "btn ghost sm"; prox.dataset.d = "1";
    prox.setAttribute("aria-label", telaT("pag.proxima"));
    prox.innerHTML = '<svg class="ico sm" viewBox="0 0 24 24"><path d="m9 6 6 6-6 6"/></svg>';
    pager.append(ant, txt, prox);
    pager.onclick = (e) => {
      const b = e.target.closest("button");
      if (!b) return;
      lista.dataset.pag = String(Number(lista.dataset.pag || 0) + Number(b.dataset.d));
      paginar(lista);
    };
    ancora.after(pager);
  }
  pager.hidden = total < 2;
  pager.querySelector("span").textContent = `${pag + 1} ${telaT("pag.de")} ${total}`;
  pager.querySelector('[data-d="-1"]').disabled = pag === 0;
  pager.querySelector('[data-d="1"]').disabled = pag >= total - 1;
}

function paginarTudo(raiz) {
  const visiveis = [];
  raiz.querySelectorAll("tbody, ul.list, ol.list").forEach((l) => {
    if (l.closest(".no-pag")) return;
    if (!tela.listas.has(l)) {
      tela.listas.add(l);
      new MutationObserver(() => { paginar(l); if (l.offsetParent) caber(l.closest(".painel")); }).observe(l, { childList: true });
    }
    l.cfN = 0;
    paginar(l);
    if (l.offsetParent) visiveis.push(l);
  });
  const painel = raiz.querySelector ? (raiz.matches(".painel") ? raiz : raiz.querySelector(".painel")) : null;
  if (visiveis.length) caber(painel);
}

// Páginas menores até a lista caber no painel (mínimo 2 itens por página; 1 em tela bem baixa).
function caber(painel) {
  if (!painel || !painel.offsetParent) return;
  const minimo = window.innerHeight < 700 ? 1 : 2;
  const listas = [...painel.querySelectorAll("tbody, ul.list, ol.list")].filter((l) => l.offsetParent && tela.listas.has(l));
  for (let volta = 0; volta < 12 && painel.scrollHeight > painel.clientHeight + 1; volta++) {
    const maior = listas
      .map((l) => ({ l, n: l.cfN || porPagina(l), itens: [...l.children].filter((c) => !c.classList.contains("pg-fora")).length }))
      .filter((x) => x.n > minimo && x.itens > minimo)
      .sort((a, b) => b.itens - a.itens)[0];
    if (!maior) break;
    maior.l.cfN = maior.n - 1;
    paginar(maior.l);
  }
}

// ---------- Confirmação dentro da página (o WebView não mostra confirm()) ----------
function confirmar(texto, botao = telaT("confirma.sim"), dica = "") {
  const d = document.getElementById("confirma-dlg");
  document.getElementById("confirma-texto").textContent = texto;
  document.getElementById("confirma-dica").textContent = dica;
  document.getElementById("confirma-dica").hidden = !dica;
  const sim = document.getElementById("confirma-sim");
  sim.textContent = botao;
  return new Promise((ok) => {
    const fim = (v) => { d.close(); ok(v); };
    sim.onclick = () => fim(true);
    document.getElementById("confirma-nao").onclick = () => fim(false);
    d.oncancel = (e) => { e.preventDefault(); fim(false); };
    d.showModal();
    document.getElementById("confirma-nao").focus();
  });
}

// ---------- Início ----------
function iniciarTela() {
  document.querySelectorAll("main > .page").forEach(montarPagina);
  montarSecoes(document.querySelector("main"));
  // A frase-guia ocupa duas linhas no celular; um toque mostra inteira.
  // Idem para as explicações longas dentro das seções.
  document.querySelectorAll(".lede, .painel .hint, .painel .note > span, .painel .aviso-fixo > span").forEach((p) => { p.addEventListener("click", () => p.classList.toggle("aberta")); });
  let espera = null;
  window.addEventListener("resize", () => {
    clearTimeout(espera);
    espera = setTimeout(() => {
      document.querySelectorAll("main > .page").forEach((p) => { ajustarSubnav(p); paginarTudo(p); });
    }, 150);
  });
}

function telaMostrouPagina(id) {
  const page = document.getElementById(id);
  if (!page) return;
  if (page.querySelector(".subnav")) irSub(id, page.dataset.sub, false);
  else paginarTudo(page);
}

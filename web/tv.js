// CamFácil IA — Modo TV (quiosque). JS puro, independente do app.js.
// A lógica dos balões é uma cópia enxuta de renderBaloes/textoBalao do app.js:
// o cliente só vê o balão com o nome da regra, nunca caixas.
"use strict";

let T = {};
const t = (k) => T[k] ?? k;
const $ = (s) => document.querySelector(s);

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v; else e.setAttribute(k, v);
  }
  for (const c of children) if (c != null) e.append(c);
  return e;
}

const POR_PAGINA = 9;
let pagina = 0;
let ultimas = [];          // câmeras do último estado
let config = { som: false, piscar: true, relogio: true };
const alertaAntes = {};
const tiles = new Map();   // id -> { raiz, img, baloes, fora }
let somCtx = null;
let avisoTimer = null;

function grade(n) {
  if (n <= 1) return [1, "g1"];
  if (n === 2) return [2, "g2"];
  if (n <= 4) return [4, "g4"];
  if (n <= 6) return [6, "g6"];
  return [9, "g9"];
}

function centro(r) {
  const p = r.pontos;
  if (r.forma === "linha") return { x: (p[0].x + p[1].x) / 2, y: (p[0].y + p[1].y) / 2 };
  return { x: p.reduce((a, q) => a + q.x, 0) / p.length, y: p.reduce((a, q) => a + q.y, 0) / p.length };
}

function textoBalao(r, e) {
  const aviso = e && e.aviso ? ` · ${e.aviso}` : "";
  if (r.forma === "zona") {
    const on = r.rotulo_on || t("regra.ocupada"), off = r.rotulo_off || t("regra.livre");
    return `${r.nome} · ${e && e.ocupada ? on : off}${aviso}`;
  }
  return `${r.nome} · ${e ? e.contagem : 0}${aviso}`;
}

const SAUDE_PROBLEMA = ["escura", "coberta", "desfocada"];

function tocarAlerta(som) {
  somCtx = somCtx || (() => { try { return new (window.AudioContext || window.webkitAudioContext)(); } catch (_) { return null; } })();
  CFSons.tocar(somCtx, som || "bipe");
}

function piscar(raiz) {
  if (!config.piscar) return;
  raiz.classList.remove("alerta"); void raiz.offsetWidth; raiz.classList.add("alerta");
}

function conferirAlertas(cam, tile) {
  let tocou = false;
  const e = cam.estado;
  const saude = e ? e.saude : "ok";
  const chaveS = `saude:${cam.id}`;
  const problema = SAUDE_PROBLEMA.includes(saude);
  if (problema && alertaAntes[chaveS] === false) { piscar(tile.raiz); if (config.som) { tocarAlerta(); tocou = true; } }
  alertaAntes[chaveS] = problema;
  const regrasLigadas = cam.regras.filter((r) => !(r.ajustes && r.ajustes.ativa === false));
  for (const r of regrasLigadas) {
    const er = e && e.regras ? e.regras[r.id] : null;
    const agora = !!(er && er.alerta_ms > 0);
    const chave = `${cam.id}:${r.id}`;
    if (agora && alertaAntes[chave] === false) { piscar(tile.raiz); if (!tocou && config.som) { tocarAlerta(r.ajustes && r.ajustes.som); tocou = true; } }
    alertaAntes[chave] = agora;
  }
}

function renderBaloes(cam, tile) {
  conferirAlertas(cam, tile);
  const e = cam.estado;
  const saude = e ? e.saude : "ok";
  const baloes = SAUDE_PROBLEMA.includes(saude)
    ? [el("div", { class: "cf-bubble is-alert saude" }, t("saude.balao." + saude))] : [];
  tile.baloes.replaceChildren(...baloes, ...regrasLigadas.map((r) => {
    const er = e && e.regras ? e.regras[r.id] : null;
    const c = centro(r);
    const b = el("div", { class: "cf-bubble" + (er && er.alerta_ms > 0 ? " is-alert" : "") }, textoBalao(r, er));
    b.style.left = `${c.x * 100}%`; b.style.top = `${c.y * 100}%`;
    return b;
  }));
  const online = !!(e && e.online);
  tile.fora.hidden = online;
  if (!online) tile.fora.textContent = e && e.saude === "sem_sinal" ? t("saude.balao.sem_sinal") : (e && e.erro) || t("estado.conectando");
}

function montar(cams) {
  const mosaico = $("#mosaico");
  const total = Math.max(1, Math.ceil(cams.length / POR_PAGINA));
  pagina = Math.min(pagina, total - 1);
  const visiveis = cams.slice(pagina * POR_PAGINA, pagina * POR_PAGINA + POR_PAGINA);
  const [, cls] = grade(visiveis.length);
  mosaico.className = "mosaico " + cls;
  const pag = $("#paginas");
  pag.hidden = total <= 1;
  pag.textContent = `${pagina + 1} / ${total} · ← →`;
  if (!visiveis.length) {
    tiles.clear();
    mosaico.replaceChildren(el("div", { class: "vazio-tela" }, t("tv.sem_cameras")));
    return;
  }
  const ids = new Set(visiveis.map((c) => c.id));
  for (const id of [...tiles.keys()]) if (!ids.has(id)) tiles.delete(id);
  mosaico.replaceChildren(...visiveis.map((cam) => {
    let tile = tiles.get(cam.id);
    if (!tile) {
      const img = el("img", { class: "video", alt: "", src: `/api/cameras/${cam.id}/mjpeg` });
      const baloes = el("div", { class: "baloes" });
      const fora = el("div", { class: "fora" });
      const raiz = el("div", { class: "cam" }, img, baloes, fora, el("div", { class: "nome" }, cam.nome));
      tile = { raiz, img, baloes, fora };
      tiles.set(cam.id, tile);
    }
    tile.raiz.querySelector(".nome").textContent = cam.nome;
    return tile.raiz;
  }));
}

async function atualizar() {
  try {
    const r = await fetch("/api/tv/estado").then((x) => x.json());
    config = r;
    const ids = (a) => a.map((c) => c.id).join(",");
    const mudou = ids(r.cameras) !== ids(ultimas) || !$("#mosaico").children.length;
    ultimas = r.cameras;
    if (mudou) montar(ultimas);
    const visiveis = ultimas.slice(pagina * POR_PAGINA, pagina * POR_PAGINA + POR_PAGINA);
    for (const cam of visiveis) { const tile = tiles.get(cam.id); if (tile) renderBaloes(cam, tile); }
    $("#relogio").hidden = !r.relogio;
    const logo = $("#logo"); const whats = $("#whats");
    if (r.logo && logo.hidden) { logo.src = "/api/tv/logo.png?" + Date.now(); logo.hidden = false; }
    if (!r.logo) logo.hidden = true;
    if (r.whatsapp && whats.hidden) { $("#qr").src = "/api/tv/whatsapp.svg?" + Date.now(); whats.hidden = false; }
    if (!r.whatsapp) whats.hidden = true;
    $("#chamada").textContent = r.chamada || (r.whatsapp ? t("tv.chamada_padrao") : "");
    $("#rodape").classList.toggle("vazio", !r.logo && !r.whatsapp);
  } catch (_) {
    // Servidor fora: mantém a última tela; os <img> MJPEG reconectam ao voltar.
  }
}

function relogio() {
  const d = new Date();
  $("#relogio").textContent = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
}

function aviso(texto, ms = 6000) {
  const a = $("#aviso");
  a.textContent = texto; a.hidden = false; a.classList.remove("sumindo");
  clearTimeout(avisoTimer);
  avisoTimer = setTimeout(() => { a.classList.add("sumindo"); setTimeout(() => { a.hidden = true; }, 450); }, ms);
}

function mudarPagina(delta) {
  const total = Math.max(1, Math.ceil(ultimas.length / POR_PAGINA));
  pagina = (pagina + delta + total) % total;
  montar(ultimas);
  for (const cam of ultimas.slice(pagina * POR_PAGINA, pagina * POR_PAGINA + POR_PAGINA)) {
    const tile = tiles.get(cam.id); if (tile) renderBaloes(cam, tile);
  }
}

function telaCheia() {
  const d = document.documentElement;
  const f = d.requestFullscreen || d.webkitRequestFullscreen;
  if (f) f.call(d).catch(() => aviso(t("tv.f11")));
}

async function main() {
  try { T = await fetch("/i18n/pt-BR.json").then((r) => r.json()); } catch (_) { /* textos padrão */ }
  document.title = t("tv.titulo");
  relogio(); setInterval(relogio, 10000);
  await atualizar(); setInterval(atualizar, 1000);
  // Páginas: setas do teclado ou do controle da TV; troca automática a cada 30 s.
  document.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight" || e.key === "ArrowDown" || e.key === "PageDown") mudarPagina(1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp" || e.key === "PageUp") mudarPagina(-1);
    else if (e.key === "f" || e.key === "F" || e.key === "Enter") telaCheia();
  });
  setInterval(() => { if (ultimas.length > POR_PAGINA) mudarPagina(1); }, 30000);
  document.addEventListener("click", telaCheia);
  let mouseTimer = null;
  document.addEventListener("mousemove", () => {
    document.body.classList.add("com-mouse");
    clearTimeout(mouseTimer); mouseTimer = setTimeout(() => document.body.classList.remove("com-mouse"), 3000);
  });
  if (!document.fullscreenElement) aviso(t("tv.f11"), 8000);
}

main();

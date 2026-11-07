// CamFácil IA — dashboard. JS puro, sem framework, embutido no binário.
"use strict";

let T = {};
const t = (k) => T[k] ?? k;
const $ = (s) => document.querySelector(s);

async function api(path, body, method) {
  const opt = body === undefined
    ? { method: method || "GET" }
    : { method: method || "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) };
  const r = await fetch(path, opt);
  const data = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(data.erro || r.statusText);
  return data;
}

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v; else e.setAttribute(k, v);
  }
  for (const c of children) if (c != null) e.append(c);
  return e;
}

function icon(id, cls = "ico sm") {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("class", cls);
  const use = document.createElementNS(ns, "use");
  use.setAttribute("href", "#" + id);
  svg.append(use);
  return svg;
}

function msg(text, kind) {
  const m = $("#rtsp-msg");
  m.textContent = text;
  m.className = "msg " + (kind || "");
}

function busy(btn, on, labelKey) {
  btn.disabled = on;
  const span = btn.querySelector("span[data-i18n]") || btn;
  span.textContent = t(on ? labelKey : span.dataset.i18n || labelKey);
}

// ---------- Navegação ----------
function showTab(id) {
  document.querySelectorAll("[role=tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === id)));
  document.querySelectorAll("[role=tabpanel]").forEach((p) => { p.hidden = p.id !== id; });
  try { localStorage.setItem("cf-tab", id); } catch (_) { /* opcional */ }
  if (id === "modulos") renderLevels();
  saudeAtivar(id === "inicio");
  eloAtivar(id === "integracoes");
  tsAtivar(id === "integracoes");
  tgAtivar(id === "integracoes");
  camerasAtivar(id === "cameras");
  ligarAtivar(id === "cameras");
}

// ---------- Visão geral ----------
let level = null;

async function loadSystem() {
  try {
    const { sistema: s, nivel } = await api("/api/system");
    level = nivel;
    const rows = [
      ["sys.so", s.os],
      ["sys.cpu", s.cpu_brand || "—"],
      ["sys.nucleos", s.cpu_cores],
      ["sys.uso", s.cpu_usage.toFixed(0) + "%"],
      ["sys.ram", `${s.ram_used_mb.toLocaleString("pt-BR")} de ${s.ram_total_mb.toLocaleString("pt-BR")} MB`],
      ["sys.self", s.self_ram_mb != null ? s.self_ram_mb + " MB" : "—"],
      ["sys.versao", s.version],
    ];
    $("#sysinfo").replaceChildren(...rows.flatMap(([k, v]) => [el("dt", {}, t(k)), el("dd", {}, String(v))]));
    $("#live-dot").className = "dot on";
    $("#live-text").textContent = t("status.online");
    $("#rail-meta").textContent = `CPU ${s.cpu_usage.toFixed(0)}% · ${s.self_ram_mb ?? "—"} MB · v${s.version}`;
  } catch (_) {
    $("#live-dot").className = "dot off";
    $("#live-text").textContent = t("status.offline");
  }
}

async function renderSelos() {
  const box = $("#selos");
  if (!box) return;
  try {
    const { selos } = await api("/api/selos");
    box.replaceChildren(...(selos.length
      ? selos.map((x) => el("span", { class: "selo" }, t("selo." + x.modulo)))
      : [el("p", { class: "hint" }, t("inicio.sem_selos"))]));
  } catch (_) { /* segue */ }
}

function renderBench(r) {
  const box = $("#bench-result");
  if (!r) { box.replaceChildren(el("p", { class: "hint" }, t("inicio.nunca"))); return; }
  const cell = (k, v, cls) => el("div", {}, el("dt", {}, t(k)), el("dd", { class: cls || "" }, String(v)));
  box.replaceChildren(
    el("dl", { class: "measure" },
      cell("inicio.nivel", t("nivel." + r.nivel), "on"),
      cell("inicio.fps", r.fps_estimado.toLocaleString("pt-BR")),
      cell("inicio.cams", r.cameras_medio)),
    el("p", { class: "hint" }, t("inicio.nota")),
  );
}

async function runBench() {
  const b = $("#bench-run");
  busy(b, true, "inicio.testando");
  try {
    const { resultado } = await api("/api/bench", {});
    level = resultado.nivel;
    renderBench(resultado);
  } catch (_) { renderBench(null); }
  busy(b, false, "inicio.teste_btn");
}

// ---------- Módulos ----------
const MODULES = [
  ["leve", [["movimento", "Movimento inteligente"], ["cerca", "Cerca e linha virtual"], ["saude", "Saúde das câmeras"]]],
  ["medio", [[null, "Pessoas e veículos"], [null, "Contagem"], [null, "Permanência"], [null, "Mapa de calor"]]],
  ["pesado", [[null, "Leitura de placas"], [null, "Reconhecimento facial"]]],
];
const RANK = { leve: 0, medio: 1, pesado: 2 };
const ESCOLA_LICAO = { cerca: "linha", movimento: "visao", saude: "saude" };

async function renderLevels() {
  let ligados = {};
  try { ligados = await api("/api/modulos"); } catch (_) { /* segue */ }
  const rows = [];
  for (const [lv, mods] of MODULES) {
    const untested = level == null && lv !== "leve";
    const locked = untested || (level != null && RANK[lv] > RANK[level]);
    for (const [id, name] of mods) {
      const disponivel = !!id && !locked;
      const stateText = untested ? t("mod.testar") : locked ? t("mod.bloqueado")
        : !id ? t("mod.precisa_modelo") : ligados[id] ? t("mod.ligado") : t("mod.desligado");
      const input = el("input", { type: "checkbox", "aria-label": name });
      if (!disponivel) input.setAttribute("disabled", "");
      if (id && ligados[id]) input.checked = true;
      input.onchange = async () => {
        try { await api("/api/modulos", { modulo: id, ligado: input.checked }); renderLevels(); } catch (e) { input.checked = !input.checked; alert(e.message); }
      };
      const sw = el("label", { class: "switch", title: stateText }, input, el("span"));
      const acoes = el("div", { class: "actions end" });
      if (id && ESCOLA_LICAO[id]) {
        const b = el("button", { class: "btn sm", type: "button", title: t("mod.aprender_dica") }, t("mod.aprender"));
        b.onclick = () => abrirEscola(ESCOLA_LICAO[id]);
        acoes.append(b);
      }
      acoes.append(sw);
      rows.push(el("tr", { class: disponivel ? "" : "locked" },
        el("td", {}, name),
        el("td", {}, t("nivel." + lv)),
        el("td", {}, el("span", { class: "state" }, locked ? icon("i-lock") : null, stateText)),
        el("td", { class: "right" }, acoes)));
    }
  }
  $("#levels").replaceChildren(...rows);
}

// ---------- Câmeras ----------
function formData() {
  const f = new FormData($("#rtsp-form"));
  return {
    fabricante: f.get("fabricante"),
    ip: String(f.get("ip") || "").trim(),
    porta: Number(f.get("porta") || 554),
    usuario: f.get("usuario") || "",
    senha: f.get("senha") || "",
    canal: Number(f.get("canal") || 1),
    stream: f.get("stream"),
  };
}

let lastUrl = "";

async function buildRtsp(ev) {
  ev.preventDefault();
  try {
    const r = await api("/api/rtsp/build", formData());
    lastUrl = r.url;
    $("#rtsp-url").textContent = r.url_sem_senha;
    $("#rtsp-out").hidden = false;
    $("#add-rtsp").hidden = false;
    msg("");
  } catch (e) { $("#rtsp-out").hidden = true; msg(e.message, "err"); }
}

async function testRtsp() {
  const d = formData();
  if (!d.ip) { msg(t("cam.ip"), "err"); $("#rtsp-form").ip.focus(); return; }
  const b = $("#rtsp-test");
  b.disabled = true; b.textContent = t("cam.testando");
  try {
    const r = await api("/api/rtsp/test", { ip: d.ip, porta: d.porta });
    msg(`${r.mensagem} (${r.ms} ms)`, r.ok ? "ok" : "err");
  } catch (e) { msg(e.message, "err"); }
  b.disabled = false; b.textContent = t("cam.testar");
}

async function copyRtsp() {
  try { await navigator.clipboard.writeText(lastUrl); msg(t("cam.copiado"), "ok"); } catch (_) { /* sem permissão */ }
}

function emptyRow(text) {
  return el("tr", { class: "empty" }, el("td", { colspan: "5" }, text));
}

// ---------- Câmeras ao vivo, balões e desenho (fase 2) ----------
let cams = [];
let camAberta = null;
let regrasAbertas = [];
let estadoTimer = null;
let desenho = { modo: "ver", pontos: [] };
let ffmpegOk = true;

function fonteNome(f) { return t("fonte." + f); }

async function carregarFfmpeg() {
  try {
    const f = await api("/api/video/ffmpeg");
    ffmpegOk = f.encontrado;
    $("#ffmpeg-aviso").hidden = f.encontrado;
    if (!f.encontrado) $("#ffmpeg-texto").textContent = `${t("cam.ffmpeg_falta")} ${f.comando_instalar}`;
  } catch (_) { /* segue */ }
}

function situacaoCam(e) {
  if (!e) return [false, t("estado.conectando")];
  if (e.online) return [true, `${t("estado.online")} · ${e.fps} q/s`];
  return [false, e.erro || t("estado.conectando")];
}

// ---------- Saúde da câmera (indicador, balão, bloco do Início) ----------
const SAUDE_PROBLEMA = ["escura", "coberta", "desfocada", "sem_sinal"];
function saudeClasse(k) { return k === "ok" ? "ok" : k === "sem_sinal" ? "off" : "warn"; }
function saudeTag(k) { return el("span", { class: "tag " + saudeClasse(k) }, t("saude." + k)); }

/// "agora", "40 s", "5 min", "2 h", "3 d".
function tempoRelativo(seg) {
  if (seg == null) return "—";
  if (seg < 5) return t("tempo.agora");
  if (seg < 60) return `${seg} ${t("tempo.s")}`;
  if (seg < 3600) return `${Math.floor(seg / 60)} ${t("tempo.min")}`;
  if (seg < 86400) return `${Math.floor(seg / 3600)} ${t("tempo.h")}`;
  return `${Math.floor(seg / 86400)} ${t("tempo.d")}`;
}

let saudeTimer = null;
async function renderSaude() {
  let r;
  try { r = await api("/api/saude"); } catch (_) { return; }
  $("#saude-desligado").hidden = r.ligado;
  const corpo = $("#saude-cameras");
  if (!r.cameras.length) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "4" }, t("saude.sem_cameras"))));
  } else {
    corpo.replaceChildren(...r.cameras.map((c) => el("tr", {},
      el("td", {}, c.nome),
      el("td", {}, saudeTag(c.saude)),
      el("td", {}, tempoRelativo(c.saude_desde_s)),
      el("td", {}, c.ultimo_problema_s == null ? t("saude.nunca") : `${tempoRelativo(c.ultimo_problema_s)} ${t("tempo.atras")}`))));
  }
  const ev = $("#saude-eventos");
  const agora = Math.floor(Date.now() / 1000);
  ev.replaceChildren(...(r.eventos.length
    ? r.eventos.map((e) => el("li", {},
      el("div", { class: "actions" }, el("strong", {}, e.camera), saudeTag(e.situacao)),
      el("span", { class: "tiny" }, new Date(e.quando * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) + ` · ${tempoRelativo(agora - e.quando)} ${t("tempo.atras")}`)))
    : [el("li", {}, el("p", {}, t("saude.sem_eventos")))]));
}
function saudeAtivar(ativo) {
  clearInterval(saudeTimer);
  saudeTimer = null;
  if (ativo) { renderSaude(); saudeTimer = setInterval(renderSaude, 5000); }
}

async function carregarCameras() {
  try { ({ cameras: cams } = await api("/api/cameras")); } catch (_) { return; }
  const corpo = $("#minhas");
  if (!cams.length) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "4" }, t("cam.sem_cameras"))));
    return;
  }
  corpo.replaceChildren(...cams.map((c) => {
    const [ok, texto] = situacaoCam(c.estado);
    const abrir = el("button", { class: "btn sm", type: "button" }, t("cam.abrir"));
    abrir.onclick = () => abrirCamera(c.id);
    const rem = el("button", { class: "btn ghost sm", type: "button" }, t("cam.remover"));
    rem.onclick = async () => {
      if (!confirm(t("cam.confirma_remover"))) return;
      await api(`/api/cameras/${c.id}`, undefined, "DELETE");
      if (camAberta === c.id) fecharCamera();
      carregarCameras();
    };
    return el("tr", {},
      el("td", {}, c.nome),
      el("td", {}, fonteNome(c.fonte)),
      el("td", {}, el("div", { class: "actions" }, el("span", { class: "state" }, el("span", { class: "dot " + (ok ? "on" : "off") }), texto), c.estado ? saudeTag(c.estado.saude) : null)),
      el("td", { class: "right" }, el("div", { class: "actions end" }, abrir, rem)));
  }));
}

async function adicionarCamera(nome, fonte, url = "") {
  try {
    const { id } = await api("/api/cameras", { nome, fonte, url });
    await carregarCameras();
    abrirCamera(id);
  } catch (e) { alert(e.message); }
}

function fecharCamera() {
  camAberta = null;
  $("#aovivo").hidden = true;
  $("#video").removeAttribute("src");
}

async function abrirCamera(id) {
  camAberta = id;
  const c = cams.find((x) => x.id === id);
  $("#aovivo").hidden = false;
  $("#aovivo-nome").textContent = c ? c.nome : "";
  $("#aovivo-saude").hidden = true;
  const somS = $("#som-saude");
  somS.checked = somLigado(`saude:${id}`);
  somS.onchange = () => { definirSom(`saude:${id}`, somS.checked); if (somS.checked) tocarAlerta(); };
  $("#aovivo-som-saude").title = t("saude.som_dica");
  $("#video").src = `/api/cameras/${id}/mjpeg?t=${Date.now()}`;
  definirModo("ver");
  await carregarRegras();
  sugAbrirCamera();
  $("#aovivo").scrollIntoView({ block: "start" });
}

async function carregarRegras() {
  if (camAberta == null) return;
  ({ regras: regrasAbertas } = await api(`/api/cameras/${camAberta}/regras`));
  desenharSalvas();
}

function centro(r) {
  const p = r.pontos;
  if (r.forma === "linha") return { x: (p[0].x + p[1].x) / 2, y: (p[0].y + p[1].y) / 2 };
  return { x: p.reduce((a, q) => a + q.x, 0) / p.length, y: p.reduce((a, q) => a + q.y, 0) / p.length };
}

function textoBalao(r, e) {
  if (r.forma === "zona") {
    const on = r.rotulo_on || t("regra.ocupada"), off = r.rotulo_off || t("regra.livre");
    return `${r.nome} · ${e && e.ocupada ? on : off}`;
  }
  return `${r.nome} · ${e ? e.contagem : 0}`;
}

// ---------- Som e piscada do alerta ----------
let somCtx = null;
const alertaAntes = {};

function somLigado(id) {
  try { return (JSON.parse(localStorage.getItem("cf-som") || "{}")[id]) !== false; } catch (_) { return true; }
}
function definirSom(id, ligado) {
  try { const m = JSON.parse(localStorage.getItem("cf-som") || "{}"); m[id] = ligado; localStorage.setItem("cf-som", JSON.stringify(m)); } catch (_) { /* opcional */ }
}

/// Dois bipes curtos, gerados na hora (sem arquivo de áudio).
function tocarAlerta() {
  try {
    somCtx = somCtx || new (window.AudioContext || window.webkitAudioContext)();
    const t0 = somCtx.currentTime;
    for (const [i, f] of [[0, 880], [0.18, 1320]]) {
      const o = somCtx.createOscillator(); const g = somCtx.createGain();
      o.type = "sine"; o.frequency.value = f;
      g.gain.setValueAtTime(0.0001, t0 + i);
      g.gain.exponentialRampToValueAtTime(0.25, t0 + i + 0.02);
      g.gain.exponentialRampToValueAtTime(0.0001, t0 + i + 0.16);
      o.connect(g).connect(somCtx.destination);
      o.start(t0 + i); o.stop(t0 + i + 0.18);
    }
  } catch (_) { /* navegador sem áudio */ }
}

function piscarTela() {
  const v = $("#viewer");
  v.classList.remove("alerta"); void v.offsetWidth; v.classList.add("alerta");
}

function conferirAlertas(estadoCam) {
  let tocou = false;
  // Saúde: alerta quando a câmera passa de ok para um problema.
  const saude = estadoCam ? estadoCam.saude : "ok";
  const chaveS = `saude:${camAberta}`;
  const problema = SAUDE_PROBLEMA.includes(saude);
  if (problema && alertaAntes[chaveS] === false) {
    piscarTela();
    if (somLigado(chaveS)) { tocarAlerta(); tocou = true; }
  }
  alertaAntes[chaveS] = problema;
  for (const r of regrasAbertas) {
    const e = estadoCam && estadoCam.regras ? estadoCam.regras[r.id] : null;
    const agora = !!(e && e.alerta_ms > 0);
    const chave = `${camAberta}:${r.id}`;
    if (agora && alertaAntes[chave] === false) {
      piscarTela();
      if (!tocou && somLigado(r.id)) { tocarAlerta(); tocou = true; }
    }
    alertaAntes[chave] = agora;
  }
}

function renderBaloes(estadoCam) {
  conferirAlertas(estadoCam);
  const camada = $("#baloes");
  const saude = estadoCam ? estadoCam.saude : "ok";
  const tag = $("#aovivo-saude");
  tag.hidden = !estadoCam;
  tag.className = "tag " + saudeClasse(saude);
  tag.textContent = t("saude." + saude);
  const baloes = SAUDE_PROBLEMA.includes(saude) && saude !== "sem_sinal"
    ? [el("div", { class: "cf-bubble is-alert saude" }, t("saude.balao." + saude))] : [];
  if (!regrasAbertas.length) { camada.replaceChildren(...baloes); return; }
  camada.replaceChildren(...baloes, ...regrasAbertas.map((r) => {
    const e = estadoCam && estadoCam.regras ? estadoCam.regras[r.id] : null;
    const c = centro(r);
    const b = el("div", { class: "cf-bubble" + (e && e.alerta_ms > 0 ? " is-alert" : "") }, textoBalao(r, e));
    b.style.left = `${c.x * 100}%`;
    b.style.top = `${c.y * 100}%`;
    return b;
  }));
  const linhas = regrasAbertas.map((r) => {
    const e = estadoCam && estadoCam.regras ? estadoCam.regras[r.id] : null;
    let agora;
    if (r.forma === "zona") agora = e && e.ocupada ? (r.rotulo_on || t("regra.ocupada")) : (r.rotulo_off || t("regra.livre"));
    else { const n = e ? e.contagem : 0; agora = `${n} ${t(n === 1 ? "regra.passagem" : "regra.passagens")}`; }
    const rem = el("button", { class: "btn ghost sm", type: "button" }, t("cam.remover"));
    rem.onclick = async () => {
      if (!confirm(t("regra.confirma_remover"))) return;
      await api(`/api/regras/${r.id}?camera_id=${camAberta}`, undefined, "DELETE");
      carregarRegras();
    };
    const chk = el("input", { type: "checkbox", "aria-label": `${t("regra.som")}: ${r.nome}` });
    chk.checked = somLigado(r.id);
    chk.onchange = () => { definirSom(r.id, chk.checked); if (chk.checked) tocarAlerta(); };
    const som = el("label", { class: "som-regra", title: t("regra.som_dica") }, el("span", { class: "switch" }, chk, el("span")), el("span", { class: "tiny" }, t("regra.som")));
    return el("tr", {}, el("td", {}, r.nome), el("td", {}, t("regra." + r.forma)), el("td", {}, agora), el("td", {}, som), el("td", { class: "right" }, rem));
  });
  $("#regras").replaceChildren(...(linhas.length ? linhas : [el("tr", { class: "empty" }, el("td", { colspan: "5" }, t("regra.nenhuma")))]));
}

async function atualizarEstado() {
  if ($("#cameras").hidden) return;
  try {
    const { cameras } = await api("/api/estado");
    if (camAberta != null) {
      const e = cameras[String(camAberta)];
      renderBaloes(e);
      const off = $("#viewer-off");
      off.hidden = !!(e && e.online);
      if (!off.hidden) off.textContent = (e && e.erro) || t("estado.conectando");
    }
  } catch (_) { /* segue */ }
}

// --- desenho ---
const svgNS = "http://www.w3.org/2000/svg";
function svgEl(tag, attrs) { const e = document.createElementNS(svgNS, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); return e; }
const pts = (lista) => lista.map((p) => `${p.x * 1000},${p.y * 1000}`).join(" ");

function desenharSalvas() {
  const svg = $("#desenho");
  svg.replaceChildren();
  // Formas salvas só aparecem no modo de desenho (para o cliente, só balões).
  if (desenho.modo !== "ver") {
    for (const r of regrasAbertas) {
      svg.append(r.forma === "linha"
        ? svgEl("polyline", { class: "forma linha salva", points: pts(r.pontos) })
        : svgEl("polygon", { class: "forma salva", points: pts(r.pontos) }));
    }
  }
  if (desenho.pontos.length) {
    const fechada = desenho.modo === "zona" && desenho.fechada;
    svg.append(fechada
      ? svgEl("polygon", { class: "forma", points: pts(desenho.pontos) })
      : svgEl("polyline", { class: "forma linha", points: pts(desenho.pontos) }));
    for (const p of desenho.pontos) svg.append(svgEl("circle", { class: "vertice", cx: p.x * 1000, cy: p.y * 1000, r: 7 }));
  }
  if (typeof sugDesenhar === "function") sugDesenhar();
}

function definirModo(m) {
  desenho = { modo: m, pontos: [], fechada: false };
  $("#viewer").classList.toggle("desenhando", m !== "ver");
  document.querySelector(`#modo input[value="${m}"]`).checked = true;
  $("#desenho-dica").textContent = t("desenho.dica_" + m);
  $("#regra-form").hidden = true;
  desenharSalvas();
}

function pontoDoClique(ev) {
  const r = $("#viewer").getBoundingClientRect();
  return { x: Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)), y: Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height)) };
}

function concluirDesenho() {
  desenho.fechada = true;
  desenharSalvas();
  const f = $("#regra-form");
  f.hidden = false;
  f.classList.toggle("zona", desenho.modo === "zona");
  f.reset();
  $("#desenho-dica").textContent = t("desenho.dica_nome");
  f.nome.focus();
}

function cliqueViewer(ev) {
  if (desenho.modo === "ver" || desenho.fechada) return;
  const p = pontoDoClique(ev);
  if (desenho.modo === "zona" && desenho.pontos.length >= 3) {
    const a = desenho.pontos[0];
    const r = $("#viewer").getBoundingClientRect();
    if (Math.hypot((p.x - a.x) * r.width, (p.y - a.y) * r.height) < 14) { concluirDesenho(); return; }
  }
  desenho.pontos.push(p);
  desenharSalvas();
  if (desenho.modo === "linha" && desenho.pontos.length === 2) concluirDesenho();
}

async function salvarRegra(ev) {
  ev.preventDefault();
  const f = new FormData($("#regra-form"));
  try {
    await api(`/api/cameras/${camAberta}/regras`, {
      nome: f.get("nome"), forma: desenho.modo, pontos: desenho.pontos,
      rotulo_on: f.get("rotulo_on") || "", rotulo_off: f.get("rotulo_off") || "",
    });
    definirModo("ver");
    await carregarRegras();
  } catch (e) { alert(e.message); }
}

function camerasAtivar(ativo) {
  clearInterval(estadoTimer);
  estadoTimer = null;
  if (!ativo) return;
  carregarFfmpeg();
  carregarCameras();
  atualizarEstado();
  let n = 0;
  estadoTimer = setInterval(() => { atualizarEstado(); if (++n % 4 === 0) carregarCameras(); }, 1000);
}

function iniciarCameras() {
  $("#add-demo").onclick = () => adicionarCamera(t("cam.demo_nome"), "demo");
  $("#add-webcam").onclick = async () => {
    let url = "";
    try { const { webcams } = await api("/api/video/webcams"); url = webcams[0] || ""; } catch (_) { /* padrão */ }
    adicionarCamera(t("cam.webcam_nome"), "webcam", url);
  };
  $("#add-rtsp").onsubmit = (ev) => { ev.preventDefault(); adicionarCamera(new FormData(ev.target).get("nome"), "rtsp", lastUrl); };
  document.querySelectorAll("#modo input").forEach((i) => { i.onchange = () => definirModo(i.value); });
  $("#viewer").addEventListener("click", cliqueViewer);
  $("#viewer").addEventListener("dblclick", () => { if (desenho.modo === "zona" && desenho.pontos.length >= 3 && !desenho.fechada) concluirDesenho(); });
  $("#regra-form").onsubmit = salvarRegra;
  $("#regra-cancelar").onclick = () => definirModo("ver");
}

// ---------- Pareamento com o site (Agente-Cam-Elo-IA) ----------
let eloTimer = null;
let eloQrLink = null;

async function renderElo() {
  let st;
  try { st = await api("/api/elo"); } catch (_) { return; }
  const dot = $("#elo-dot");
  let texto;
  if (st.pareado) { dot.className = "dot on"; texto = `${t("elo.pareado")} ${st.dono_email || ""}`; }
  else if (st.conectado) { dot.className = "dot"; texto = t("elo.conectado"); }
  else if (st.ativo) { dot.className = "dot"; texto = t("elo.conectando"); }
  else { dot.className = "dot"; texto = t("elo.desligado"); }
  $("#elo-status").textContent = texto;
  $("#elo-impressao").textContent = st.impressao;
  $("#elo-site").textContent = st.site.replace(/^https?:\/\//, "");

  const temQr = !st.pareado && st.link && st.expira_em_s > 0;
  $("#elo-qr-area").hidden = !temQr;
  if (temQr && eloQrLink !== st.link) { eloQrLink = st.link; $("#elo-qr").src = "/api/elo/qr.svg?t=" + Date.now(); }
  if (temQr) {
    // Código para digitar no site, em blocos de 4 (o site ignora os espaços).
    const cod = new URLSearchParams(st.link.split("#")[1] || "").get("c") || "";
    $("#elo-codigo").textContent = cod.match(/.{1,4}/g)?.join(" ") || "";
    const m = Math.floor(st.expira_em_s / 60), s = String(st.expira_em_s % 60).padStart(2, "0");
    $("#elo-expira").textContent = `${m}:${s}`;
  }
  $("#elo-pedido").hidden = !st.pedido;
  if (st.pedido) $("#elo-pedido-email").textContent = st.pedido.email;
  $("#elo-parear").hidden = st.pareado;
  $("#elo-parear").textContent = t(temQr ? "elo.novo_qr" : "elo.parear");
  $("#elo-desparear").hidden = !st.pareado;
  const m = $("#elo-msg");
  m.textContent = st.erro && !st.pareado && st.ativo ? st.erro : "";
  m.className = "msg" + (st.erro && !st.conectado ? " err" : "");
}

function eloMsg(texto, tipo) { const m = $("#elo-msg"); m.textContent = texto; m.className = "msg " + (tipo || ""); }

function eloAtivar(ativo) {
  clearInterval(eloTimer);
  eloTimer = null;
  if (ativo) { renderElo(); eloTimer = setInterval(renderElo, 1500); }
}

async function eloAcao(fn) {
  try { await fn(); await renderElo(); } catch (e) { eloMsg(e.message, "err"); }
}


// ---------- Acessar de fora (Tailscale) ----------
let tsTimer = null;
let tsQrUrl = null;
let tsUltimo = null;

async function renderTailscale() {
  let st;
  try { st = await api("/api/tailscale"); } catch (_) { return; }
  tsUltimo = st;
  const dot = $("#ts-dot");
  let texto;
  if (!st.instalado) { dot.className = "dot"; texto = t("ts.nao_instalado"); }
  else if (!st.logado) { dot.className = "dot off"; texto = t("ts.nao_logado"); }
  else if (st.lan) { dot.className = "dot on"; texto = t("ts.lan"); }
  else if (st.aguardando) { dot.className = "dot"; texto = t("ts.aguardando"); }
  else { dot.className = "dot on"; texto = t("ts.pronto"); }
  $("#ts-status").textContent = texto;
  const end = $("#ts-endereco");
  if (st.url) {
    const copiar = el("button", { class: "btn ghost sm", type: "button" }, icon("i-copy"), el("span", {}, t("cam.copiar")));
    copiar.onclick = async () => { try { await navigator.clipboard.writeText(st.url); tsMsg(t("ts.copiado"), "ok"); } catch (_) { /* sem permissão */ } };
    end.replaceChildren(el("div", { class: "actions" }, el("code", { class: "mono" }, st.url), copiar));
  } else end.textContent = "—";
  $("#ts-nome").textContent = st.nome || "—";
  const temQr = !!st.url && st.logado;
  $("#ts-qr-area").hidden = !temQr;
  if (temQr && tsQrUrl !== st.url) { tsQrUrl = st.url; $("#ts-qr").src = "/api/tailscale/qr.svg?t=" + Date.now(); }
  $("#ts-instalar").hidden = st.logado;
  if (!st.logado) {
    $("#ts-instalar-texto").textContent = t(st.instalado ? "ts.logar_texto" : "ts.instalar_texto");
    $("#ts-comando").textContent = st.instalado ? (st.sistema === "windows" ? "" : "sudo tailscale up") : st.comando_instalar;
    $("#ts-comando").parentElement.hidden = !$("#ts-comando").textContent;
  }
  const m = $("#ts-msg");
  if (st.erro && st.instalado && !st.logado) { m.textContent = st.erro; m.className = "msg"; }
  else if (m.dataset.fixo !== "1") { m.textContent = ""; m.className = "msg"; }
}

function tsMsg(texto, tipo) { const m = $("#ts-msg"); m.textContent = texto; m.className = "msg " + (tipo || ""); m.dataset.fixo = "1"; setTimeout(() => { m.dataset.fixo = ""; }, 4000); }

function tsAtivar(ativo) {
  clearInterval(tsTimer);
  tsTimer = null;
  if (ativo) { renderTailscale(); tsTimer = setInterval(renderTailscale, 10000); }
}

// ---------- Telegram ----------
let tgTimer = null;

function tgMsg(texto, tipo) { const m = $("#tg-msg"); m.textContent = texto; m.className = "msg " + (tipo || ""); }

async function renderTelegram() {
  let r;
  try { r = await api("/api/telegram"); } catch (e) { tgMsg(e.message, "err"); return; }
  const s = r.situacao;
  const dot = $("#tg-dot");
  let texto;
  if (!s.configurado) { dot.className = "dot"; texto = t("tg.sem_token"); }
  else if (s.escutando) { dot.className = "dot on"; texto = t("tg.escutando"); }
  else { dot.className = "dot off"; texto = s.ultimo_erro || t("tg.parado"); }
  $("#tg-status").textContent = texto;
  $("#tg-bot").textContent = s.bot ? "@" + s.bot : "—";
  $("#tg-envios").textContent = s.configurado
    ? `${s.enviados} ${t("tg.enviados")} · ${s.na_fila} ${t("tg.fila")}${s.descartados ? ` · ${s.descartados} ${t("tg.descartados")}` : ""}` : "—";
  $("#tg-acoes").hidden = !s.configurado;
  $("#tg-contatos").hidden = !s.configurado;
  $("#tg-form").hidden = s.configurado;
  const f = $("#tg-fotos"); f.checked = s.fotos;
  $("#tg-fotos-wrap").title = t("tg.fotos_dica");
  if (s.configurado && s.ultimo_erro && !s.escutando) { /* já mostrado na situação */ }
  const corpo = $("#tg-chats");
  if (!r.chats.length) { corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "5" }, t("tg.nenhum_contato")))); return; }
  corpo.replaceChildren(...r.chats.map((c) => {
    const chk = el("input", { type: "checkbox", "aria-label": `${t("tg.col_autorizado")}: ${c.nome}` });
    chk.checked = c.autorizado;
    chk.onchange = async () => { try { await api(`/api/telegram/chats/${c.chat_id}`, { autorizado: chk.checked }); renderTelegram(); } catch (e) { chk.checked = !chk.checked; tgMsg(e.message, "err"); } };
    const rem = el("button", { class: "btn ghost sm", type: "button" }, t("tg.remover_contato"));
    rem.onclick = async () => { if (!confirm(t("tg.confirma_remover_contato"))) return; await api(`/api/telegram/chats/${c.chat_id}`, undefined, "DELETE"); renderTelegram(); };
    return el("tr", {},
      el("td", {}, c.nome || "—"),
      el("td", { class: "mono" }, String(c.chat_id)),
      el("td", {}, `${tempoRelativo(Math.floor(Date.now() / 1000) - c.visto_em)} ${t("tempo.atras")}`),
      el("td", {}, el("label", { class: "switch" }, chk, el("span"))),
      el("td", { class: "right" }, rem));
  }));
}

async function tgSalvar(ev) {
  ev.preventDefault();
  const f = ev.target; const b = $("#tg-salvar");
  const token = f.token.value.trim();
  if (!token) return;
  busy(b, true, "tg.salvando"); tgMsg("");
  try {
    const r = await api("/api/telegram", { token });
    f.token.value = "";
    tgMsg(`${t("tg.salvo")} @${r.bot || r.nome}`, "ok");
    renderTelegram();
  } catch (e) { tgMsg(e.message, "err"); }
  busy(b, false, "tg.salvar");
}

async function tgTestar() {
  const b = $("#tg-testar");
  busy(b, true, "tg.testando"); tgMsg("");
  try {
    const r = await api("/api/telegram/testar", {});
    tgMsg(`${t("tg.testado")} ${r.enviados} ${r.enviados === 1 ? t("tg.contato") : t("tg.contatos_n")} (@${r.bot}).`, "ok");
  } catch (e) { tgMsg(e.message, "err"); }
  busy(b, false, "tg.testar");
  renderTelegram();
}

function tgAtivar(ativo) {
  clearInterval(tgTimer);
  tgTimer = null;
  if (ativo) { renderTelegram(); tgTimer = setInterval(renderTelegram, 5000); }
}

// ---------- Aula guiada (agente guia) ----------
// Aprender fica no site (escolinha): aqui é só operação.
const ESCOLA = "https://cfia.ysafe.com.br/aprender";
function abrirEscola(licao) { window.open(`${ESCOLA}#${licao || ""}`, "_blank", "noopener"); }

// ---------- Ligar câmera: procurar → senha → funcionando ----------
let ligarAlvo = null;
let ligarProcurou = false;

async function ligarVerFfmpeg() {
  try {
    const f = await api("/api/video/ffmpeg");
    $("#ligar-ffmpeg").hidden = f.encontrado;
    return f.encontrado;
  } catch (_) { return true; }
}

async function ligarInstalarFfmpeg() {
  const b = $("#ligar-ffmpeg-btn"); const m = $("#ligar-ffmpeg-msg");
  b.disabled = true; m.className = "msg"; m.textContent = t("ligar.ffmpeg_instalando");
  try {
    await api("/api/video/ffmpeg", {});
    m.className = "msg ok"; m.textContent = t("ligar.ffmpeg_ok");
    setTimeout(ligarVerFfmpeg, 1500);
  } catch (e) { m.className = "msg err"; m.textContent = e.message; }
  b.disabled = false;
}

async function ligarProcurar(autoescolher = true) {
  ligarProcurou = true;
  const st = $("#ligar-status"); const lista = $("#ligar-lista"); const b = $("#discover");
  b.disabled = true;
  st.replaceChildren(el("span", { class: "carregando", "aria-hidden": "true" }), t("ligar.procurando"));
  lista.replaceChildren();
  try {
    const { cameras, rede } = await api("/api/procurar");
    if (!cameras.length) {
      st.textContent = t("ligar.achadas");
      lista.replaceChildren(el("li", { class: "feita" }, el("p", {}, t("ligar.nenhuma") + (rede ? ` ${t("ligar.rede")} ${rede}` : ""))));
    } else {
      st.textContent = t("ligar.achadas");
      lista.replaceChildren(...cameras.map((c) => {
        const desc = [c.fabricante, c.modelo].filter(Boolean).join(" · ") || "RTSP";
        const li = el("li", c.ja_adicionada ? { class: "feita" } : { tabindex: "0", role: "button" },
          el("div", {}, el("strong", { class: "mono" }, c.ip), el("p", {}, desc)),
          c.ja_adicionada ? el("span", { class: "badge" }, t("ligar.ja")) : el("span", { class: "btn sm" }, t("ligar.escolher")));
        if (!c.ja_adicionada) {
          const ir = () => ligarEscolher(c.ip, c.fabricante);
          li.onclick = ir;
          li.onkeydown = (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); ir(); } };
        }
        return li;
      }));
      const livres = cameras.filter((c) => !c.ja_adicionada);
      if (autoescolher && livres.length === 1) ligarEscolher(livres[0].ip, livres[0].fabricante);
    }
  } catch (e) { st.textContent = e.message; }
  b.disabled = false;
}

function ligarEscolher(ip, fabricante) {
  ligarAlvo = { ip: ip.trim(), fabricante: fabricante || null };
  $("#ligar-ip").textContent = ligarAlvo.ip;
  $("#ligar-passo2").hidden = false;
  $("#ligar-ok").hidden = true;
  $("#ligar-form").hidden = false;
  $("#ligar-msg").textContent = "";
  $("#ligar-passo2").scrollIntoView({ block: "nearest" });
  $("#ligar-senha").focus();
}

async function ligarConectar(ev) {
  ev.preventDefault();
  if (!ligarAlvo) return;
  const f = ev.target; const m = $("#ligar-msg"); const b = $("#ligar-conectar");
  b.disabled = true; m.className = "msg"; m.textContent = t("ligar.conectando");
  $("#ligar-ok").hidden = true;
  try {
    const r = await api("/api/conectar", {
      ip: ligarAlvo.ip, fabricante: ligarAlvo.fabricante,
      usuario: f.usuario.value.trim(), senha: f.senha.value, nome: f.nome.value.trim() || null,
    });
    f.senha.value = ""; f.nome.value = "";
    m.textContent = "";
    $("#ligar-ok-texto").textContent = `${r.nome} ${t("ligar.ligada")}`;
    $("#ligar-ok").hidden = false;
    $("#ligar-form").hidden = true;
    await carregarCameras();
    abrirCamera(r.id);
    ligarProcurar(false);
  } catch (e) { m.className = "msg err"; m.textContent = e.message; }
  b.disabled = false;
}

function ligarResetar() {
  $("#ligar-passo2").hidden = true;
  $("#ligar-ok").hidden = true;
  $("#ligar-form").hidden = false;
  ligarAlvo = null;
}

function iniciarLigar() {
  $("#discover").onclick = () => { ligarResetar(); ligarProcurar(); };
  $("#ligar-ffmpeg-btn").onclick = ligarInstalarFfmpeg;
  $("#ligar-form").onsubmit = ligarConectar;
  $("#ligar-manual").onsubmit = (ev) => { ev.preventDefault(); ligarEscolher(ev.target.ip.value, null); };
  $("#ligar-outra").onclick = () => { ligarResetar(); $("#ligar-passo1").scrollIntoView({ block: "start" }); };
  $("#ligar-aula").onclick = () => abrirEscola("linha");
  $("#ligar-demo").onclick = () => adicionarCamera(t("cam.demo_nome"), "demo");
}

/// Ao abrir a tela Câmeras: confere o ffmpeg e, na primeira vez, já procura.
function ligarAtivar(ativo) {
  if (!ativo) return;
  ligarVerFfmpeg();
  if (!ligarProcurou) ligarProcurar();
}

// ---------- IA, sugira para mim ----------
let sugTimer = null;
let sugPreview = [];

function sugDesenhar() {
  // Prévia tracejada das sugestões por cima do vídeo (só para o técnico).
  const svg = $("#desenho");
  svg.querySelectorAll(".sugestao").forEach((e) => e.remove());
  for (const sg of sugPreview) {
    svg.append(sg.forma === "linha"
      ? svgEl("polyline", { class: "forma linha sugestao", points: pts(sg.pontos) })
      : svgEl("polygon", { class: "forma sugestao", points: pts(sg.pontos) }));
  }
}

function sugRender(r) {
  const lista = $("#sug-lista");
  sugPreview = r.sugestoes || [];
  sugDesenhar();
  $("#sug-andamento").hidden = !r.iniciado;
  if (!r.iniciado) { lista.replaceChildren(); return; }
  $("#sug-barra").style.width = `${Math.round((r.progresso || 0) * 100)}%`;
  const pessoas = `${r.pessoas} ${r.pessoas === 1 ? t("sug.pessoa") : t("sug.pessoas")}`;
  $("#sug-status").textContent = r.terminou ? `${t("sug.pronto")} · ${pessoas}` : `${t("sug.olhando")} ${r.restante_s} s · ${pessoas}`;
  if (!sugPreview.length) {
    lista.replaceChildren(el("li", { class: "tiny" }, r.terminou ? t("sug.nada") : t("sug.aguarde")));
    return;
  }
  lista.replaceChildren(...sugPreview.map((sg) => {
    const aceitar = el("button", { class: "btn primary sm", type: "button" }, t("sug.aceitar"));
    aceitar.onclick = async () => {
      aceitar.disabled = true;
      try {
        await api(`/api/cameras/${camAberta}/regras`, { nome: sg.nome, forma: sg.forma, pontos: sg.pontos, rotulo_on: "", rotulo_off: "" });
        await carregarRegras();
        sugPreview = sugPreview.filter((x) => x !== sg);
        aceitar.closest("li").replaceChildren(el("span", { class: "msg ok" }, `${sg.nome} ${t("sug.criada")}`));
        sugDesenhar();
      } catch (e) { alert(e.message); aceitar.disabled = false; }
    };
    const pct = Math.round(sg.confianca * 100);
    return el("li", {},
      el("div", { class: "sug-info" },
        el("strong", {}, `${sg.nome} · ${t("regra." + sg.forma)}`),
        el("p", { class: "tiny" }, sg.motivo),
        el("span", { class: "badge" }, `${t("sug.confianca")} ${pct}%`)),
      aceitar);
  }));
}

async function sugAtualizar() {
  if (camAberta == null) return;
  try {
    const r = await api(`/api/cameras/${camAberta}/sugestoes`);
    sugRender(r);
    if (r.iniciado && !r.terminou) sugTimer = setTimeout(sugAtualizar, 2000);
    else { $("#sug-iniciar").disabled = false; }
  } catch (_) { /* segue */ }
}

async function sugIniciar() {
  if (camAberta == null) return;
  clearTimeout(sugTimer);
  $("#sug-iniciar").disabled = true;
  try { await api(`/api/cameras/${camAberta}/sugestoes`, { segundos: 60 }); sugAtualizar(); }
  catch (e) { alert(e.message); $("#sug-iniciar").disabled = false; }
}

function sugAbrirCamera() {
  clearTimeout(sugTimer);
  sugPreview = [];
  $("#sug-lista").replaceChildren();
  $("#sug-andamento").hidden = true;
  $("#sug-iniciar").disabled = false;
  sugAtualizar();
}

// ---------- Abrir junto com o computador ----------
async function iniciarAutoinicio() {
  const c = $("#autoinicio"); const m = $("#auto-msg");
  try { c.checked = (await api("/api/autoinicio")).ligado; } catch (_) { /* segue */ }
  c.onchange = async () => {
    c.disabled = true; m.className = "msg"; m.textContent = "";
    try {
      await api("/api/autoinicio", { ligado: c.checked });
      m.className = "msg ok"; m.textContent = t(c.checked ? "auto.ligado" : "auto.desligado");
    } catch (e) { c.checked = !c.checked; m.className = "msg err"; m.textContent = e.message; }
    c.disabled = false;
  };
}

// ---------- Isenção ----------
async function disclaimer() {
  try {
    const { aceito } = await api("/api/disclaimer");
    if (aceito) return;
    $("#disc-list").replaceChildren(...(T["isencao.itens"] || []).map((i) => el("li", {}, i)));
    const d = $("#disclaimer");
    d.showModal();
    d.addEventListener("cancel", (e) => e.preventDefault());
    $("#disc-ok").onclick = async () => { await api("/api/disclaimer", {}); d.close(); };
  } catch (_) { /* segue sem bloquear */ }
}

// ---------- Início ----------
async function main() {
  T = await fetch("/i18n/pt-BR.json").then((r) => r.json());
  document.querySelectorAll("[data-i18n]").forEach((e) => { e.textContent = t(e.dataset.i18n); });
  document.querySelectorAll("[data-i18n-ph]").forEach((e) => { e.placeholder = t(e.dataset.i18nPh); });

  document.querySelectorAll("[role=tab]").forEach((b) => { b.onclick = () => showTab(b.dataset.tab); });
  $("#bench-run").onclick = runBench;
  $("#rtsp-form").onsubmit = buildRtsp;
  $("#rtsp-test").onclick = testRtsp;
  $("#rtsp-copy").onclick = copyRtsp;
  iniciarCameras();
  iniciarLigar();
  iniciarAutoinicio();
  $("#sug-iniciar").onclick = sugIniciar;
  $("#elo-parear").onclick = () => eloAcao(() => api("/api/elo/codigo", {}));
  $("#elo-aceitar").onclick = () => eloAcao(() => api("/api/elo/confirmar", { aceito: true }));
  $("#elo-recusar").onclick = () => eloAcao(() => api("/api/elo/confirmar", { aceito: false }));
  $("#tg-form").onsubmit = tgSalvar;
  $("#tg-testar").onclick = tgTestar;
  $("#tg-remover").onclick = async () => { if (!confirm(t("tg.confirma_remover"))) return; try { await api("/api/telegram", undefined, "DELETE"); tgMsg(t("tg.removido"), "ok"); renderTelegram(); } catch (e) { tgMsg(e.message, "err"); } };
  $("#tg-fotos").onchange = async (e) => { try { await api("/api/telegram/fotos", { ligado: e.target.checked }); } catch (err) { tgMsg(err.message, "err"); } };
  // O bot escreve a hora no fuso do técnico: informa o do navegador.
  api("/api/fuso", { minutos: -new Date().getTimezoneOffset() }).catch(() => {});
  $("#ts-copiar-cmd").onclick = async () => { try { await navigator.clipboard.writeText($("#ts-comando").textContent); tsMsg(t("ts.cmd_copiado"), "ok"); } catch (_) { /* sem permissão */ } };
  $("#elo-desparear").onclick = () => { if (confirm(t("elo.confirma_desparear"))) eloAcao(() => api("/api/elo/desparear", {})); };

  const { fabricantes } = await api("/api/rtsp/vendors");
  $("#vendor").replaceChildren(...fabricantes.map((f) =>
    el("option", { value: f.id }, f.tambem.length ? `${f.nome} (mesmo padrão: ${f.tambem.join(", ")})` : f.nome)));

  const { resultado } = await api("/api/bench");
  renderBench(resultado);
  renderSelos();

  let tab = "inicio";
  try { tab = localStorage.getItem("cf-tab") || tab; } catch (_) { /* opcional */ }
  // Sem câmera nenhuma: o programa abre direto em "Ligar câmera".
  try { const { cameras } = await api("/api/cameras"); if (!cameras.length) tab = "cameras"; } catch (_) { /* segue */ }
  await loadSystem();
  showTab(tab);
  disclaimer();
  setInterval(loadSystem, 15000);
}

main();

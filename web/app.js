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
  eloAtivar(id === "integracoes");
  camerasAtivar(id === "cameras");
  ligarAtivar(id === "ligar");
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
const AULAS = { cerca: true };

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
      if (id && AULAS[id]) {
        const b = el("button", { class: "btn sm", type: "button" }, t("mod.aula"));
        b.onclick = () => aulaIniciar(id);
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

async function discover() {
  const b = $("#discover"); const body = $("#found");
  busy(b, true, "cam.buscando");
  try {
    const { cameras } = await api("/api/discover");
    if (!cameras.length) body.replaceChildren(emptyRow(t("cam.nenhuma")));
    else body.replaceChildren(...cameras.map((c) => {
      const use = el("button", { class: "btn sm", type: "button" }, t("cam.usar"));
      use.onclick = () => {
        const f = $("#rtsp-form");
        f.ip.value = c.ip;
        const v = (c.fabricante || "").toLowerCase();
        if ([...f.fabricante.options].some((o) => o.value === v)) f.fabricante.value = v;
        f.senha.focus();
      };
      return el("tr", {},
        el("td", { class: "mono" }, c.ip),
        el("td", {}, c.fabricante || "—"),
        el("td", {}, c.modelo || c.nome || "—"),
        el("td", { class: "mono" }, c.mac || "—"),
        el("td", { class: "right" }, use));
    }));
  } catch (e) { body.replaceChildren(emptyRow(e.message)); }
  busy(b, false, "cam.busca_btn");
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
  if (e.online) {
    const extra = e.saude ? ` · ${t("saude." + e.saude)}` : "";
    return [true, `${t("estado.online")} · ${e.fps} q/s${extra}`];
  }
  return [false, e.erro || t("estado.conectando")];
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
      el("td", {}, el("span", { class: "state" }, el("span", { class: "dot " + (ok ? "on" : "off") }), texto)),
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
  $("#video").src = `/api/cameras/${id}/mjpeg?t=${Date.now()}`;
  definirModo("ver");
  await carregarRegras();
  if (!aula) $("#aovivo").scrollIntoView({ block: "start" });
  aulaEvento("camera_aberta");
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

function renderBaloes(estadoCam) {
  const camada = $("#baloes");
  if (!regrasAbertas.length) { camada.replaceChildren(); return; }
  camada.replaceChildren(...regrasAbertas.map((r) => {
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
    return el("tr", {}, el("td", {}, r.nome), el("td", {}, t("regra." + r.forma)), el("td", {}, agora), el("td", { class: "right" }, rem));
  });
  $("#regras").replaceChildren(...(linhas.length ? linhas : [el("tr", { class: "empty" }, el("td", { colspan: "4" }, t("regra.nenhuma")))]));
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
    aulaEvento("regra_" + (regrasAbertas.at(-1) || {}).forma);
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


// ---------- Aula guiada (agente guia) ----------
let aula = null; // { dados, i }
let aulaAudio = null;

function aulaLimparAlvo() { document.querySelectorAll(".guia-alvo").forEach((e) => e.classList.remove("guia-alvo")); }

function aulaMostrar() {
  aulaLimparAlvo();
  if (!aula) { $("#aula").hidden = true; return; }
  const p = aula.dados.passos[aula.i];
  if (p.aba && document.querySelector(`[data-tab="${p.aba}"]`).getAttribute("aria-selected") !== "true") showTab(p.aba);
  $("#aula").hidden = false;
  $("#aula-titulo").textContent = aula.dados.titulo;
  $("#aula-passo").textContent = `${aula.i + 1}/${aula.dados.passos.length}`;
  $("#aula-texto").textContent = p.texto;
  const ultimo = aula.i === aula.dados.passos.length - 1;
  const b = $("#aula-proximo");
  b.textContent = t(ultimo ? "aula.concluir" : p.espera ? "aula.esperando" : "aula.proximo");
  b.disabled = !!p.espera;
  // O cartão entra na página logo acima do alvo, para nunca cobrir o vídeo.
  const card = $("#aula");
  const alvo = p.alvo ? document.querySelector(p.alvo) : null;
  if (alvo) {
    alvo.classList.add("guia-alvo");
    // Antes do cabeçalho do bloco: instrução, depois os controles, depois o vídeo.
    const ancora = alvo.closest(".block-head") || alvo;
    ancora.insertAdjacentElement("beforebegin", card);
    card.classList.add("inline");
    requestAnimationFrame(() => card.scrollIntoView({ block: "start" }));
  } else {
    document.body.append(card);
    card.classList.remove("inline");
  }
  aulaTocar();
}

function aulaTocar() {
  if (!aula) return;
  const p = aula.dados.passos[aula.i];
  try { if (aulaAudio) aulaAudio.pause(); } catch (_) { /* ok */ }
  aulaAudio = new Audio(`/aulas/${aula.dados.modulo}-${p.id}.ogg`);
  aulaAudio.play().catch(() => { /* sem áudio gravado: segue só com texto */ });
}

function aulaAvancar() {
  if (!aula) return;
  if (aula.i >= aula.dados.passos.length - 1) { aulaSair(); return; }
  aula.i += 1;
  aulaMostrar();
}

function aulaSair() {
  try { if (aulaAudio) aulaAudio.pause(); } catch (_) { /* ok */ }
  aula = null;
  aulaMostrar();
}

function aulaEvento(nome) {
  if (!aula) return;
  const p = aula.dados.passos[aula.i];
  if (p.espera === nome) aulaAvancar();
}

async function aulaIniciar(modulo) {
  const dados = await fetch(`/aulas/${modulo}.json`).then((r) => r.json());
  aula = { dados, i: 0 };
  aulaMostrar();
}

// O passo 5 espera o selo: verifica de tempos em tempos.
setInterval(async () => {
  if (!aula || aula.dados.passos[aula.i].espera !== "selo_cerca") return;
  try { const { selos } = await api("/api/selos"); if (selos.some((x) => x.modulo === "cerca")) aulaEvento("selo_cerca"); } catch (_) { /* segue */ }
}, 2000);

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

async function ligarProcurar() {
  ligarProcurou = true;
  const st = $("#ligar-status"); const lista = $("#ligar-lista"); const b = $("#ligar-procurar");
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
      if (livres.length === 1) ligarEscolher(livres[0].ip, livres[0].fabricante);
    }
  } catch (e) { st.textContent = e.message; }
  b.disabled = false;
}

function ligarEscolher(ip, fabricante) {
  ligarAlvo = { ip: ip.trim(), fabricante: fabricante || null };
  $("#ligar-ip").textContent = ligarAlvo.ip;
  $("#ligar-passo2").hidden = false;
  $("#ligar-pronto").hidden = true;
  $("#ligar-msg").textContent = "";
  $("#ligar-passo2").scrollIntoView({ block: "nearest" });
  $("#ligar-senha").focus();
}

async function ligarConectar(ev) {
  ev.preventDefault();
  if (!ligarAlvo) return;
  const f = ev.target; const m = $("#ligar-msg"); const b = $("#ligar-conectar");
  b.disabled = true; m.className = "msg"; m.textContent = t("ligar.conectando");
  try {
    const r = await api("/api/conectar", {
      ip: ligarAlvo.ip, fabricante: ligarAlvo.fabricante,
      usuario: f.usuario.value.trim(), senha: f.senha.value, nome: f.nome.value.trim() || null,
    });
    f.senha.value = ""; f.nome.value = "";
    m.textContent = "";
    $("#ligar-passo2").hidden = true;
    $("#ligar-pronto").hidden = false;
    $("#ligar-pronto").dataset.id = r.id;
    $("#ligar-video").src = `/api/cameras/${r.id}/mjpeg?t=${Date.now()}`;
    $("#ligar-endereco").textContent = `${r.nome} · ${r.endereco}`;
    $("#ligar-pronto").scrollIntoView({ block: "start" });
    carregarCameras();
    ligarProcurar();
  } catch (e) { m.className = "msg err"; m.textContent = e.message; }
  b.disabled = false;
}

function ligarSair() { $("#ligar-video").removeAttribute("src"); }

function iniciarLigar() {
  $("#ligar-procurar").onclick = ligarProcurar;
  $("#ligar-ffmpeg-btn").onclick = ligarInstalarFfmpeg;
  $("#ligar-form").onsubmit = ligarConectar;
  $("#ligar-manual").onsubmit = (ev) => { ev.preventDefault(); ligarEscolher(ev.target.ip.value, null); };
  $("#ligar-outra").onclick = () => { $("#ligar-pronto").hidden = true; ligarSair(); $("#ligar-passo1").scrollIntoView({ block: "start" }); };
  $("#ligar-ver").onclick = () => { const id = Number($("#ligar-pronto").dataset.id); ligarSair(); showTab("cameras"); abrirCamera(id); };
  $("#ligar-aula").onclick = () => { ligarSair(); aulaIniciar("cerca"); };
  $("#ligar-demo").onclick = async () => { ligarSair(); showTab("cameras"); await adicionarCamera(t("cam.demo_nome"), "demo"); };
}

function ligarAtivar(ativo) {
  if (!ativo) { ligarSair(); return; }
  ligarVerFfmpeg();
  if (!ligarProcurou) ligarProcurar();
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
  $("#discover").onclick = discover;
  iniciarCameras();
  iniciarLigar();
  $("#elo-parear").onclick = () => eloAcao(() => api("/api/elo/codigo", {}));
  $("#elo-aceitar").onclick = () => eloAcao(() => api("/api/elo/confirmar", { aceito: true }));
  $("#elo-recusar").onclick = () => eloAcao(() => api("/api/elo/confirmar", { aceito: false }));
  $("#elo-desparear").onclick = () => { if (confirm(t("elo.confirma_desparear"))) eloAcao(() => api("/api/elo/desparear", {})); };
  $("#found").replaceChildren(emptyRow(t("cam.vazio")));

  const { fabricantes } = await api("/api/rtsp/vendors");
  $("#vendor").replaceChildren(...fabricantes.map((f) =>
    el("option", { value: f.id }, f.tambem.length ? `${f.nome} (mesmo padrão: ${f.tambem.join(", ")})` : f.nome)));

  const { resultado } = await api("/api/bench");
  renderBench(resultado);
  renderSelos();
  $("#aula-proximo").onclick = aulaAvancar;
  $("#aula-ouvir").onclick = aulaTocar;
  $("#aula-sair").onclick = aulaSair;

  let tab = "inicio";
  try { tab = localStorage.getItem("cf-tab") || tab; } catch (_) { /* opcional */ }
  // Sem câmera nenhuma: o programa abre direto em "Ligar câmera".
  try { const { cameras } = await api("/api/cameras"); if (!cameras.length) tab = "ligar"; } catch (_) { /* segue */ }
  await loadSystem();
  showTab(tab);
  disclaimer();
  setInterval(loadSystem, 15000);
}

main();

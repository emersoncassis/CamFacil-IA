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
  document.querySelectorAll("[role=tab][data-tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === id)));
  document.querySelectorAll("[role=tabpanel]").forEach((p) => { p.hidden = p.id !== id; });
  try { localStorage.setItem("cf-tab", id); } catch (_) { /* opcional */ }
  if (id === "modulos") renderLevels();
  iaAtivar(id === "modulos");
  saudeAtivar(id === "inicio");
  eloAtivar(id === "integracoes");
  tsAtivar(id === "integracoes");
  tgAtivar(id === "integracoes");
  camerasAtivar(id === "cameras");
  redeAtivar(id === "rede");
  ligarAtivar(id === "cameras");
  manutencaoAtivar(id === "manutencao");
  telaMostrouPagina(id);
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
  ["leve", [["movimento", "Movimento inteligente"], ["cerca", "Cerca e linha virtual"], ["saude", "Saúde das câmeras"], ["camera", "Alertas da câmera"]]],
  ["medio", [["pessoas", "Pessoas e veículos"], ["contagem", "Contagem"], ["permanencia", "Permanência"], ["calor", "Mapa de calor"]]],
  ["pesado", [[null, "Leitura de placas"], [null, "Reconhecimento facial"]]],
];
const RANK = { leve: 0, medio: 1, pesado: 2 };
const ESCOLA_LICAO = { cerca: "linha", movimento: "visao", saude: "saude" };
let modulosInfo = {};

async function renderLevels() {
  let ligados = {};
  try { ligados = await api("/api/modulos"); } catch (_) { /* segue */ }
  modulosInfo = ligados;
  const rows = [];
  for (const [lv, mods] of MODULES) {
    const untested = level == null && lv !== "leve";
    const locked = untested || (level != null && RANK[lv] > RANK[level]);
    for (const [id, name] of mods) {
      const disponivel = !!id && !locked;
      const stateText = !id ? t("mod.pesado_texto") : untested ? t("mod.testar") : locked ? t("mod.bloqueado")
        : lv === "medio" && !ligados.ia_pronta ? (ligados[id] ? t("mod.baixar_modelo") : t("mod.desligado"))
        : ligados[id] ? t("mod.ligado") : t("mod.desligado");
      const input = el("input", { type: "checkbox", "aria-label": name });
      if (!disponivel) input.setAttribute("disabled", "");
      if (id && ligados[id]) input.checked = true;
      input.onchange = async () => {
        try {
          await api("/api/modulos", { modulo: id, ligado: input.checked });
          // Primeiro módulo médio ligado sem modelo: começa o download.
          if (input.checked && lv === "medio" && !ligados.ia_pronta) { await api("/api/ia/baixar", {}); }
          renderLevels(); renderIa();
        } catch (e) { input.checked = !input.checked; alert(e.message); }
      };
      const sw = el("label", { class: "switch", title: stateText }, input, el("span"));
      const acoes = el("div", { class: "actions end" });
      if (id && ESCOLA_LICAO[id]) {
        const b = el("button", { class: "btn sm", type: "button", title: t("mod.aprender_dica") }, t("mod.aprender"));
        b.onclick = () => abrirEscola(ESCOLA_LICAO[id]);
        acoes.append(b);
      }
      if (id === "permanencia") {
        const n = el("input", { class: "perm-input mono", type: "number", min: "5", max: "3600", value: String(ligados.permanencia_s || 60), "aria-label": t("mod.permanencia_s") });
        n.onchange = async () => { try { await api("/api/ia/permanencia", { segundos: Number(n.value) }); } catch (e) { alert(e.message); } };
        acoes.append(el("span", { class: "perm-wrap" }, n, t("mod.permanencia_s")));
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

// ---------- Modelo de IA do nível médio (download sob demanda) ----------
let iaTimer = null;
function iaMsg(texto, tipo) { const m = $("#ia-msg"); m.textContent = texto; m.className = "msg " + (tipo || ""); }

async function renderIa() {
  let r;
  try { r = await api("/api/ia"); } catch (_) { return; }
  const a = r.andamento; const dot = $("#ia-dot");
  let texto;
  if (!r.suportado) { dot.className = "dot off"; texto = `${t("ia.nao_suportado")} (${r.plataforma})`; }
  else if (r.baixando) { dot.className = "dot"; texto = t("ia.fase." + a.fase) || a.fase; }
  else if (r.pronto) { dot.className = "dot on"; texto = `${t("ia.pronto")} (ONNX Runtime ${r.onnxruntime.versao} · ${r.modelo.nome})`; }
  else if (a.fase === "erro" || a.fase === "cancelado") { dot.className = "dot off"; texto = `${t("ia.fase." + a.fase)}: ${a.erro || ""}`; }
  else { dot.className = "dot"; texto = t("ia.faltando"); }
  $("#ia-status").textContent = texto;
  const d = r.detector;
  $("#ia-detector").textContent = d.erro ? d.erro : d.carregado
    ? `${t("ia.detector_ok")} ${d.inferencias} ${t("ia.detector_inferencias")} ${d.ultimo_ms} ${t("ia.detector_ms")}` : t("ia.detector_parado");
  $("#ia-pasta").textContent = r.pasta;
  $("#ia-baixar").hidden = r.pronto || r.baixando || !r.suportado;
  $("#ia-cancelar").hidden = !r.baixando;
  $("#ia-apagar").hidden = !r.pronto || r.baixando;
  const and = $("#ia-andamento");
  and.hidden = !r.baixando;
  if (r.baixando) {
    const pct = a.total ? Math.round(a.baixado / a.total * 100) : 0;
    $("#ia-barra").style.width = `${a.fase === "extraindo" ? 100 : pct}%`;
    const mb = (n) => (n / 1048576).toFixed(1).replace(".", ",");
    $("#ia-andamento-texto").textContent = a.total ? `${mb(a.baixado)} de ${mb(a.total)} MB (${pct}%)` : `${mb(a.baixado)} MB`;
  }
  if (r.pronto && modulosInfo.ia_pronta === false) renderLevels();
}

function iaAtivar(ativo) {
  clearInterval(iaTimer);
  iaTimer = null;
  if (ativo) { renderIa(); iaTimer = setInterval(renderIa, 1500); }
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
  if (e.online) return [true, `${t("estado.online")} · ${Math.round(e.fps * 10) / 10} q/s`];
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
  if (ativo) { renderSaude(); redeResumoInicio(); saudeTimer = setInterval(renderSaude, 5000); }
}

async function carregarCameras() {
  try { ({ cameras: cams } = await api("/api/cameras")); } catch (_) { return; }
  const corpo = $("#minhas");
  if (!cams.length) {
    corpo.replaceChildren(el("li", { class: "empty" }, el("p", {}, t("cam.sem_cameras"))));
    return;
  }
  corpo.replaceChildren(...cams.map((c) => {
    const [ok, texto] = c.ativa === false ? [false, t("cam.inativa")] : situacaoCam(c.estado);
    const abrir = el("button", { class: "btn sm", type: "button" }, icon("i-play"), el("span", {}, t("cam.abrir")));
    abrir.onclick = () => abrirCamera(c.id);
    abrir.disabled = c.ativa === false;
    const editar = el("button", { class: "btn sm", type: "button", "aria-label": `${t("cam.editar")}: ${c.nome}` }, icon("i-edit"), el("span", {}, t("cam.editar")));
    editar.onclick = () => editarCamera(c.id);
    const rem = el("button", { class: "btn sm perigo", type: "button", "aria-label": `${t("cam.remover")}: ${c.nome}` }, icon("i-trash"), el("span", {}, t("cam.remover")));
    rem.onclick = () => removerCamera(c.id);
    return el("li", { class: "cam-item" },
      el("div", { class: "cam-info" },
        el("strong", {}, c.nome),
        el("div", { class: "cam-meta" },
          el("span", { class: "tiny" }, fonteNome(c.fonte)),
          el("span", { class: "state" }, el("span", { class: "dot " + (ok ? "on" : "off") }), texto),
          c.ativa !== false && c.estado ? saudeTag(c.estado.saude) : null)),
      el("div", { class: "actions cam-acoes" }, abrir, editar, rem));
  }));
}

// ---------- Editar e remover câmera ----------
let camEditando = null;
let camNomeTocado = false;   // o técnico mexeu no nome neste diálogo?
let camNomesCanais = null;   // promessa: nomes reais dos canais do gravador
let camCanalEspera = null;

/// Trocou o canal no Editar câmera: sugere o nome que o gravador dá ao canal.
function camCanalMudou() {
  clearTimeout(camCanalEspera);
  camCanalEspera = setTimeout(async () => {
    const c = camEditando; const f = $("#cam-form");
    const n = Number(f.canal.value);
    if (!c || !c.canal || !n || n === c.canal) { $("#cam-canal-sug").hidden = true; return; }
    if (!camNomesCanais) camNomesCanais = api(`/api/cameras/${c.id}/nomes-canais`).then((r) => r.nomes || {}).catch(() => ({}));
    const nomes = await camNomesCanais;
    if (camEditando !== c || Number(f.canal.value) !== n) return;
    // Sem nome no gravador: mantém o padrão "<gravador> · Canal N".
    const base = (c.nome.match(/^(.*) · Canal \d+$/) || [])[1];
    const sug = nomes[String(n)] || (base ? `${base} · ${t("gravador.canal_n").replace("{n}", n)}` : "");
    if (!sug) { $("#cam-canal-sug").hidden = true; return; }
    if (!camNomeTocado) f.nome.value = sug;
    $("#cam-canal-sug-txt").textContent = t(nomes[String(n)] ? "camed.nome_gravador" : "camed.nome_padrao").replace("{nome}", sug);
    $("#cam-canal-sug-txt").title = $("#cam-canal-sug-txt").textContent;
    $("#cam-canal-sug-usar").hidden = f.nome.value === sug;
    $("#cam-canal-sug-usar").onclick = () => { f.nome.value = sug; $("#cam-canal-sug-usar").hidden = true; };
    $("#cam-canal-sug").hidden = false;
  }, 250);
}

function camFonteMudou() {
  const f = $("#cam-form").fonte.value;
  $("#cam-url-campo").hidden = f === "demo";
  $("#cam-senha-campo").hidden = f !== "rtsp";
  // Autocorreção de IP só existe para RTSP (demo, webcam e arquivo não têm rede).
  $("#cam-autoip-campo").hidden = f !== "rtsp";
  // Canal só para câmera que veio de gravador.
  $("#cam-canal-campo").hidden = f !== "rtsp" || !(camEditando && camEditando.canal);
  $("#cam-url-rotulo").textContent = t("camed.url_" + (f === "demo" ? "rtsp" : f));
  $("#cam-form").url.placeholder = f === "rtsp" ? "rtsp://admin:***@192.168.1.108:554/…" : "";
}

function editarCamera(id) {
  const c = cams.find((x) => x.id === id);
  if (!c) return;
  camEditando = c;
  const f = $("#cam-form");
  f.nome.value = c.nome;
  f.fonte.value = c.fonte;
  f.url.value = c.url_sem_senha || "";
  f.senha.value = "";
  f.ativa.checked = c.ativa !== false;
  f.canal.value = c.canal || "";
  camNomeTocado = false;
  camNomesCanais = null;
  $("#cam-canal-sug").hidden = true;
  // Sem MAC aprendido a autocorreção não tem como reconhecer a câmera: trava.
  f.auto_ip.checked = !!c.auto_ip;
  f.auto_ip.disabled = !c.mac_conhecido;
  $("#cam-autoip-campo").classList.toggle("travado", !c.mac_conhecido);
  $("#cam-autoip-ajuda").textContent = t(c.mac_conhecido ? "autoip.ajuda" : "autoip.aguardando");
  $("#cam-dlg-msg").textContent = "";
  $("#cam-dlg-msg").className = "msg";
  camFonteMudou();
  $("#cam-dlg").showModal();
}

async function salvarCamera(ev) {
  ev.preventDefault();
  const c = camEditando;
  if (!c) return;
  const f = $("#cam-form"); const m = $("#cam-dlg-msg"); const b = $("#cam-dlg-salvar");
  const corpo = { nome: f.nome.value.trim(), fonte: f.fonte.value, ativa: f.ativa.checked };
  if (corpo.fonte !== "demo" && (f.url.value.trim() !== (c.url_sem_senha || "") || corpo.fonte !== c.fonte)) corpo.url = f.url.value.trim();
  if (corpo.fonte === "rtsp" && f.senha.value) corpo.senha = f.senha.value;
  if (corpo.fonte === "rtsp" && c.canal && Number(f.canal.value) && Number(f.canal.value) !== c.canal) {
    corpo.canal = Number(f.canal.value);
    delete corpo.url; // o servidor troca o canal no endereço
  }
  if (corpo.fonte === "rtsp" && !f.auto_ip.disabled && f.auto_ip.checked !== !!c.auto_ip) corpo.auto_ip = f.auto_ip.checked;
  if (!corpo.nome) { m.className = "msg err"; m.textContent = t("cam.nome_ph"); f.nome.focus(); return; }
  b.disabled = true; m.className = "msg"; m.textContent = t("camed.salvando");
  try {
    const r = await api(`/api/cameras/${c.id}`, corpo, "PATCH");
    f.senha.value = "";
    $("#cam-dlg").close();
    await carregarCameras();
    if (camAberta === c.id) {
      if (r.ativa) {
        $("#aovivo-nome").textContent = r.nome;
        streamMostrar(cams.find((x) => x.id === c.id));
        $("#video").src = urlAoVivo(c.id);
      } else fecharCamera();
    }
  } catch (e) { m.className = "msg err"; m.textContent = e.message; }
  b.disabled = false;
}

async function removerCamera(id) {
  const c = cams.find((x) => x.id === id);
  const nome = c ? c.nome : "";
  if (!(await confirmar(`${t("cam.remover")} ${nome}?`, t("cam.remover_sim"), t("cam.remover_dica")))) return false;
  try {
    await api(`/api/cameras/${id}`, undefined, "DELETE");
  } catch (e) { alert(e.message); return false; }
  if (camAberta === id) fecharCamera();
  await carregarCameras();
  return true;
}

async function adicionarCamera(nome, fonte, url = "") {
  try {
    const { id } = await api("/api/cameras", { nome, fonte, url });
    await carregarCameras();
    abrirCamera(id);
  } catch (e) { alert(e.message); }
}

// ---------- Stream da visualização (extra × principal) ----------
// Só muda o que se vê: a IA continua no stream extra (leveza).
function streamEscolhido(id) {
  try { return (JSON.parse(localStorage.getItem("cf-stream") || "{}")[id]) === "principal" ? "principal" : "extra"; } catch (_) { return "extra"; }
}
function guardarStream(id, s) {
  try { const m = JSON.parse(localStorage.getItem("cf-stream") || "{}"); if (s === "principal") m[id] = s; else delete m[id]; localStorage.setItem("cf-stream", JSON.stringify(m)); } catch (_) { /* opcional */ }
}
/// Vale pedir o principal? (só com padrão de caminho conhecido)
function streamPrincipalAtivo(c) { return !!c && c.stream_principal === true && streamEscolhido(c.id) === "principal"; }
function urlAoVivo(id) {
  const c = cams.find((x) => x.id === id);
  return `/api/cameras/${id}/mjpeg?${streamPrincipalAtivo(c) ? "stream=principal&" : ""}t=${Date.now()}`;
}

function streamMostrar(c) {
  const b = $("#video-stream"); const dica = $("#video-stream-dica");
  dica.hidden = true;
  // Não se aplica: demo/webcam/arquivo, ou a câmera já está no principal.
  if (!c || c.stream_principal == null) { b.hidden = true; return; }
  b.hidden = false;
  const principal = streamPrincipalAtivo(c);
  const travado = c.stream_principal === false;
  const atual = t(principal ? "stream.principal" : "stream.extra");
  $("#video-stream-txt").textContent = atual;
  b.setAttribute("aria-pressed", String(principal));
  b.setAttribute("aria-disabled", String(travado));
  b.setAttribute("aria-label", travado ? `${t("stream.rotulo")}: ${atual}. ${t("stream.sem_padrao")}`
    : t("stream.botao").replace("{atual}", atual).replace("{outro}", t(principal ? "stream.extra" : "stream.principal")));
  b.title = travado ? t("stream.sem_padrao") : t("stream.ia_extra");
  b.onclick = async (ev) => {
    ev.stopPropagation();
    if (travado) { dica.textContent = t("stream.sem_padrao"); dica.hidden = !dica.hidden; return; }
    if (!principal && !(await confirmar(t("stream.pergunta"), t("stream.usar_principal"), t("stream.aviso")))) return;
    guardarStream(c.id, principal ? "extra" : "principal");
    streamMostrar(c);
    $("#video").src = urlAoVivo(c.id);
  };
}

function fecharCamera() {
  camAberta = null;
  $("#aovivo").hidden = true;
  $("#video").removeAttribute("src");
  if (document.getElementById("cameras").dataset.sub === "aovivo") irSub("cameras", "suas");
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
  streamMostrar(c);
  $("#video").src = urlAoVivo(id);
  $("#aovivo-objetos").hidden = true;
  try { modulosInfo = await api("/api/modulos"); } catch (_) { /* segue */ }
  calorConfigurar();
  definirModo("ver");
  await carregarRegras();
  sugAbrirCamera();
  irSub("cameras", "aovivo");
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
  const aviso = e && e.aviso ? ` · ${e.aviso}` : "";
  if (r.forma === "zona") {
    const on = r.rotulo_on || t("regra.ocupada"), off = r.rotulo_off || t("regra.livre");
    return `${r.nome} · ${e && e.ocupada ? on : off}${aviso}`;
  }
  return `${r.nome} · ${e ? e.contagem : 0}${aviso}`;
}

// ---------- Nível médio no ao vivo: o que a IA viu e mapa de calor ----------
let calorLigado = false;
let calorTimer = null;

function renderObjetos(estadoCam) {
  const tag = $("#aovivo-objetos");
  const obj = estadoCam && estadoCam.objetos ? estadoCam.objetos : [];
  const vivo = estadoCam && estadoCam.ia_ms != null && estadoCam.ia_ms < 5000;
  tag.hidden = !vivo;
  tag.className = "tag ia";
  tag.textContent = `${t("aovivo.visto")}: ${obj.length ? obj.map((o) => o[0]).join(" · ") : "—"}`;
}

function calorDesenhar(c) {
  const svg = $("#desenho");
  svg.querySelectorAll(".calor").forEach((e) => e.remove());
  if (!calorLigado || !c || !c.maximo) return;
  const cw = 1000 / c.colunas, ch = 1000 / c.linhas;
  for (let y = 0; y < c.linhas; y++) for (let x = 0; x < c.colunas; x++) {
    const v = c.celulas[y * c.colunas + x];
    if (!v) continue;
    const a = Math.min(0.75, 0.12 + 0.63 * (v / c.maximo));
    svg.append(svgEl("rect", { class: "calor", x: x * cw, y: y * ch, width: cw, height: ch, fill: `rgba(6, 203, 63, ${a.toFixed(2)})` }));
  }
}

async function calorAtualizar() {
  if (camAberta == null || !calorLigado) { calorDesenhar(null); return; }
  try { const { calor } = await api(`/api/cameras/${camAberta}/calor`); calorDesenhar(calor); } catch (_) { /* segue */ }
}

function calorConfigurar() {
  const wrap = $("#calor-wrap"); const chk = $("#calor-toggle");
  wrap.hidden = !modulosInfo.calor;
  wrap.title = t("calor.dica");
  chk.checked = calorLigado;
  chk.onchange = () => { calorLigado = chk.checked; calorAtualizar(); };
  clearInterval(calorTimer);
  calorTimer = setInterval(calorAtualizar, 3000);
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

/// Toca o som escolhido (bipe, sino, alarme, gota), gerado na hora.
function tocarAlerta(som) {
  somCtx = somCtx || (() => { try { return new (window.AudioContext || window.webkitAudioContext)(); } catch (_) { return null; } })();
  CFSons.tocar(somCtx, som || "bipe");
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
      if (!tocou && somLigado(r.id)) { tocarAlerta(r.ajustes && r.ajustes.som); tocou = true; }
    }
    alertaAntes[chave] = agora;
  }
}

/// "Passagem · carro, moto" / "Permanência · pessoa" / "Zona" (regras antigas).
function tipoDaRegra(r) {
  const a = r.ajustes || {};
  if (!a.intel || a.intel === "legado") return t("regra." + r.forma);
  const nomes = (a.classes || []).map((c) => {
    const k = catalogo && catalogo.classes.find((x) => x.indice === c);
    return k ? t("classe." + k.id).toLowerCase() : t("regra.classe_estudo");
  });
  return [t("intel." + a.intel), nomes.join(", ")].filter(Boolean).join(" · ");
}

function renderBaloes(estadoCam) {
  conferirAlertas(estadoCam);
  const camada = $("#baloes");
  const saude = estadoCam ? estadoCam.saude : "ok";
  const tag = $("#aovivo-saude");
  tag.hidden = !estadoCam;
  tag.className = "tag " + saudeClasse(saude);
  tag.textContent = t("saude." + saude);
  renderObjetos(estadoCam);
  const baloes = SAUDE_PROBLEMA.includes(saude) && saude !== "sem_sinal"
    ? [el("div", { class: "cf-bubble is-alert saude" }, t("saude.balao." + saude))] : [];
  if (!regrasAbertas.length) {
    camada.replaceChildren(...baloes);
    if (!$("#regras").querySelector(".empty")) $("#regras").replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "5" }, t("regra.nenhuma"))));
    return;
  }
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
      if (!(await confirmar(t("regra.confirma_remover"), t("regra.remover_sim")))) return;
      await api(`/api/regras/${r.id}?camera_id=${camAberta}`, undefined, "DELETE");
      carregarRegras();
    };
    const chk = el("input", { type: "checkbox", "aria-label": `${t("regra.som")}: ${r.nome}` });
    chk.checked = somLigado(r.id);
    chk.onchange = () => { definirSom(r.id, chk.checked); if (chk.checked) tocarAlerta(r.ajustes && r.ajustes.som); };
    const som = el("label", { class: "som-regra", title: t("regra.som_dica") }, el("span", { class: "switch" }, chk, el("span")), el("span", { class: "tiny" }, t("regra.som")));
    const edt = el("button", { class: "btn ghost sm", type: "button" }, t("regra.editar"));
    edt.onclick = () => editarRegra(r);
    return el("tr", {}, el("td", {}, r.nome), el("td", {}, tipoDaRegra(r)), el("td", {}, agora), el("td", {}, som), el("td", { class: "right" }, edt, rem));
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

let mostrarFormas = false;
try { mostrarFormas = localStorage.getItem("cf-formas") === "1"; } catch (_) { /* opcional */ }

function desenharSalvas() {
  const svg = $("#desenho");
  svg.replaceChildren();
  // Formas salvas aparecem no modo de desenho. No modo Ver, só se o operador
  // ligar "Mostrar linhas e zonas" (padrão: só os balões, regra 4).
  if (desenho.modo !== "ver" || mostrarFormas) {
    for (const r of regrasAbertas) {
      const cor = r.ajustes && r.ajustes.cor ? ` cor-${r.ajustes.cor}` : "";
      svg.append(r.forma === "linha"
        ? svgEl("polyline", { class: "forma linha salva" + cor, points: pts(r.pontos) })
        : svgEl("polygon", { class: "forma salva" + cor, points: pts(r.pontos) }));
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
  if (calorLigado) calorAtualizar();
}

function fecharFormRegra() {
  regraEditando = null;
  $("#regra-form").hidden = true;
  const b = $("#regra-form").querySelector('button[type="submit"]');
  if (b) b.textContent = t("regra.salvar");
}

function definirModo(m) {
  regraEditando = null;
  const bt = $("#regra-form").querySelector('button[type="submit"]');
  if (bt) bt.textContent = t("regra.salvar");
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

async function concluirDesenho() {
  desenho.fechada = true;
  desenharSalvas();
  const f = $("#regra-form");
  f.hidden = false;
  f.classList.toggle("zona", desenho.modo === "zona");
  f.reset();
  $("#desenho-dica").textContent = t("desenho.dica_nome");
  f.nome.focus();
  await montarFormRegra();
}

// ---------- Inteligência, quem, cor e som da regra (SDD-09) ----------
// Uma regra faz uma coisa só. A lista mostra o peso de cada uma e só deixa
// marcar o que está disponível (módulo ligado); "Movimento" é o aviso simples
// de sempre, que não precisa de nada.
let catalogo = null;
let regraEditando = null;
const CORES_PADRAO = "verde";

/// Abre o mesmo formulário já preenchido. A inteligência fica fixa; classes,
/// tempo, textos, cor e som mudam.
async function editarRegra(r) {
  regraEditando = r;
  desenho = { modo: "ver", pontos: [], fechada: true };
  const f = $("#regra-form");
  f.hidden = false;
  f.classList.toggle("zona", r.forma === "zona");
  f.reset();
  f.nome.value = r.nome;
  f.rotulo_on.value = r.rotulo_on || "";
  f.rotulo_off.value = r.rotulo_off || "";
  f.querySelector('button[type="submit"]').textContent = t("regra.salvar_edicao");
  await montarFormRegra(r.forma, r);
  f.nome.focus();
}

async function carregarCatalogo() {
  try { catalogo = await api("/api/inteligencias"); } catch (_) { /* usa o último */ }
  return catalogo;
}

function abaRegra(nome) {
  document.querySelectorAll("#regra-abas .aba-regra").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.aba === nome)));
  document.querySelectorAll("#regra-form .aba-painel").forEach((p) => { p.hidden = p.dataset.painel !== nome; });
}

function intelEscolhida() {
  const r = document.querySelector('#intel-lista input[name="intel"]:checked');
  return r ? r.value : "legado";
}

function atualizarParametro() {
  const id = intelEscolhida();
  const i = catalogo && catalogo.inteligencias.find((x) => x.id === id);
  const wrap = $("#param-wrap");
  const legado = id === "legado";
  document.querySelectorAll("#classes-lista input").forEach((c) => { c.disabled = legado; });
  if (!i || !i.parametro) { wrap.hidden = true; return; }
  wrap.hidden = false;
  $("#param-rotulo").textContent = t(i.parametro.unidade === "quantos" ? "regra.param_quantos" : "regra.param_segundos");
  const inp = wrap.querySelector("input");
  inp.min = i.parametro.min; inp.max = i.parametro.max; inp.value = i.parametro.padrao;
}

async function montarFormRegra(forma = desenho.modo, regra = null) {
  await carregarCatalogo();
  abaRegra("intel");
  const cat = catalogo || { inteligencias: [], classes: [], sons: ["bipe"], cores: [CORES_PADRAO] };
  const da_forma = cat.inteligencias.filter((i) => i.forma === forma);
  const itens = [...da_forma, { id: "legado", peso: "leve", disponivel: true }];
  const atual = regra ? ((regra.ajustes && regra.ajustes.intel) || "legado") : null;
  const primeira = atual || (itens.find((i) => i.disponivel) || itens[itens.length - 1]).id;
  $("#intel-lista").replaceChildren(...itens.map((i) => {
    const radio = el("input", { type: "radio", name: "intel", value: i.id });
    radio.disabled = !i.disponivel || (atual !== null && i.id !== atual);
    radio.checked = i.id === primeira;
    radio.onchange = atualizarParametro;
    const chave = i.id === "legado" ? "legado" : i.id;
    const nome = i.id === "legado" ? t("regra.tipo_legado_nome") : t("intel." + i.id);
    const desc = i.id === "legado" ? t("regra.tipo_legado_desc") : t("intel." + i.id + ".desc");
    const extra = i.disponivel ? "" : t("regra.modulo_desligado").replace("{modulo}", t("regra.modulo_nome." + i.modulo));
    return el("label", { class: "intel-item" + (i.disponivel ? "" : " off"), "data-intel": chave },
      radio,
      el("span", { class: "intel-txt" },
        el("strong", {}, nome),
        el("span", { class: "tiny" }, extra || desc)),
      el("span", { class: "peso peso-" + i.peso }, t("peso." + i.peso)));
  }));
  // Quem: pessoa marcada de início.
  $("#classes-lista").replaceChildren(...cat.classes.map((c) => {
    const chk = el("input", { type: "checkbox", value: c.indice });
    chk.checked = regra ? ((regra.ajustes && regra.ajustes.classes) || []).includes(c.indice) : c.indice === 0;
    return el("label", { class: "chip" }, chk, el("span", {}, t("classe." + c.id)));
  }));
  if (cat.estudo) $("#classes-lista").append(el("p", { class: "tiny span-all" }, t("regra.estudo_dica")));
  // Cor e som.
  $("#cores-lista").replaceChildren(...cat.cores.map((c) => {
    const radio = el("input", { type: "radio", name: "cor", value: c, "aria-label": t("cor." + c) });
    radio.checked = c === ((regra && regra.ajustes && regra.ajustes.cor) || CORES_PADRAO);
    return el("label", { class: "cor-item cor-" + c, title: t("cor." + c) }, radio, el("span", {}));
  }));
  const sel = $("#som-escolha");
  sel.replaceChildren(...cat.sons.map((id) => el("option", { value: id }, t("som." + id))));
  sel.value = (regra && regra.ajustes && regra.ajustes.som) || "bipe";
  atualizarParametro();
  if (regra && regra.ajustes && regra.ajustes.parametro > 0) $("#param-wrap input").value = regra.ajustes.parametro;
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
  if (regraEditando) {
    const ed = regraEditando;
    const cls = [...document.querySelectorAll("#classes-lista input:checked")].map((c) => Number(c.value));
    const legado = !ed.ajustes || !ed.ajustes.intel || ed.ajustes.intel === "legado";
    if (!legado && !cls.length && !(catalogo && catalogo.estudo)) { abaRegra("quem"); alert(t("regra.escolha_quem")); return; }
    const corE = (document.querySelector('#cores-lista input:checked') || {}).value || CORES_PADRAO;
    const pE = Number(f.get("parametro"));
    const corpo = { camera_id: camAberta, nome: f.get("nome"), rotulo_on: f.get("rotulo_on") || "", rotulo_off: f.get("rotulo_off") || "", cor: corE, som: f.get("som") || "bipe" };
    if (!legado) { corpo.classes = cls; if (!$("#param-wrap").hidden && pE > 0) corpo.parametro = pE; }
    try {
      await api(`/api/regras/${ed.id}`, corpo, "PATCH");
      fecharFormRegra();
      await carregarRegras();
    } catch (e) { alert(e.message); }
    return;
  }
  const intel = intelEscolhida();
  const classes = [...document.querySelectorAll("#classes-lista input:checked")].map((c) => Number(c.value));
  if (intel !== "legado" && !classes.length && !(catalogo && catalogo.estudo)) {
    abaRegra("quem");
    alert(t("regra.escolha_quem"));
    return;
  }
  const cor = (document.querySelector('#cores-lista input:checked') || {}).value || CORES_PADRAO;
  const p = Number(f.get("parametro"));
  try {
    await api(`/api/cameras/${camAberta}/regras`, {
      nome: f.get("nome"), forma: desenho.modo, pontos: desenho.pontos,
      rotulo_on: f.get("rotulo_on") || "", rotulo_off: f.get("rotulo_off") || "",
      intel: intel === "legado" ? "" : intel,
      classes: intel === "legado" ? [] : classes,
      parametro: !$("#param-wrap").hidden && p > 0 ? p : 0,
      cor, som: f.get("som") || "bipe",
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
  document.querySelectorAll("#regra-abas .aba-regra").forEach((b) => { b.onclick = () => abaRegra(b.dataset.aba); });
  $("#som-prova").onclick = () => tocarAlerta($("#som-escolha").value);
  carregarCatalogo();
  $("#regra-cancelar").onclick = () => definirModo("ver");
  $("#cam-form").onsubmit = salvarCamera;
  $("#cam-form").nome.addEventListener("input", () => { camNomeTocado = true; });
  $("#cam-form").canal.addEventListener("input", camCanalMudou);
  $("#cam-form").fonte.onchange = camFonteMudou;
  $("#cam-dlg-cancelar").onclick = () => $("#cam-dlg").close();
  $("#cam-dlg-remover").onclick = async () => {
    const c = camEditando;
    if (!c) return;
    $("#cam-dlg").close();
    if (!(await removerCamera(c.id))) editarCamera(c.id);
  };
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
    rem.onclick = async () => { if (!await confirmar(t("tg.confirma_remover_contato"))) return; await api(`/api/telegram/chats/${c.chat_id}`, undefined, "DELETE"); renderTelegram(); };
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
          const ir = () => ligarEscolher(c.ip, c.fabricante, c.mac, c.modelo);
          li.onclick = ir;
          li.onkeydown = (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); ir(); } };
        }
        return li;
      }));
      const livres = cameras.filter((c) => !c.ja_adicionada);
      if (autoescolher && livres.length === 1) ligarEscolher(livres[0].ip, livres[0].fabricante, livres[0].mac, livres[0].modelo);
    }
  } catch (e) { st.textContent = e.message; }
  b.disabled = false;
}

// Login padrão das câmeras: guardado cifrado no programa, a tela só sabe o usuário.
let loginSalvo = null; let usarLoginSalvo = false; let ultimoLogin = null;
async function carregarLogin() {
  try { const r = await api("/api/login-cameras"); loginSalvo = r.salvo ? r.usuario : null; } catch (_) { loginSalvo = null; }
  usarLoginSalvo = !!loginSalvo;
  mostrarLogin();
}
function mostrarLogin() {
  $("#ligar-salvo").hidden = !usarLoginSalvo;
  $("#ligar-salvo-usuario").textContent = loginSalvo || "";
  document.querySelectorAll("#ligar-form [data-login]").forEach((l) => { l.hidden = usarLoginSalvo; });
}

function ligarEscolher(ip, fabricante, mac, modelo) {
  ligarAlvo = { ip: ip.trim(), fabricante: fabricante || null, mac: mac || null, modelo: modelo || null };
  $("#ligar-gravador").hidden = true;
  $("#ligar-ip").textContent = ligarAlvo.ip;
  $("#ligar-passo2").hidden = false;
  $("#ligar-ok").hidden = true;
  $("#ligar-form").hidden = false;
  $("#ligar-msg").textContent = "";
  mostrarLogin();
  if (usarLoginSalvo) $("#ligar-conectar").focus(); else $("#ligar-senha").focus();
}

async function ligarConectar(ev) {
  ev.preventDefault();
  if (!ligarAlvo) return;
  const f = ev.target; const m = $("#ligar-msg"); const b = $("#ligar-conectar");
  b.disabled = true; m.className = "msg"; m.textContent = t("ligar.conectando");
  $("#ligar-ok").hidden = true;
  try {
    const login = usarLoginSalvo ? null : { usuario: f.usuario.value.trim(), senha: f.senha.value };
    const r = await api("/api/conectar", {
      ip: ligarAlvo.ip, fabricante: ligarAlvo.fabricante, usar_salvo: usarLoginSalvo,
      usuario: login ? login.usuario : "", senha: login ? login.senha : "", nome: f.nome.value.trim() || null,
      auto_ip: f.auto_ip.checked, mac: ligarAlvo.mac, modelo: ligarAlvo.modelo,
    });
    if (r.tipo === "gravador") {
      // DVR/NVR: o técnico escolhe os canais antes de criar as câmeras.
      m.textContent = "";
      f.senha.value = "";
      gravadorMostrar(r, f.auto_ip.checked, login);
      b.disabled = false;
      return;
    }
    // Login digitado e que funcionou: pergunta se vale para as outras câmeras.
    ultimoLogin = login;
    $("#ligar-perguntar").hidden = !login;
    f.senha.value = ""; f.nome.value = ""; f.auto_ip.checked = false;
    m.textContent = "";
    $("#ligar-ok-texto").textContent = `${r.nome} ${t("ligar.ligada")}` + (r.auto_ip && !r.mac_conhecido ? ` ${t("autoip.ligar_depois")}` : "");
    $("#ligar-ok").hidden = false;
    $("#ligar-form").hidden = true;
    await carregarCameras();
    abrirCamera(r.id);
    ligarProcurar(false);
  } catch (e) { m.className = "msg err"; m.textContent = e.message; }
  b.disabled = false;
}

// ---------- Gravador (DVR/NVR): escolher os canais ----------
let gravadorAtual = null;

function gravadorConta() {
  const n = gravadorAtual ? gravadorAtual.escolhidos.size : 0;
  $("#ligar-gravador-conta").textContent = t("gravador.conta").replace("{n}", n);
  $("#ligar-gravador-add").disabled = !n;
  $("#ligar-gravador-dica").textContent = t("gravador.dica").replace("{nome}", $("#ligar-gravador-nome").value.trim() || "…");
  gravadorPlaceholders();
}

function gravadorMostrar(r, autoIp, login) {
  // Começa só com o canal 1: cada canal é um vídeo a mais no PC.
  gravadorAtual = { token: r.token, autoIp, login, escolhidos: new Set([1]) };
  $("#ligar-form").hidden = true;
  $("#ligar-ok").hidden = true;
  $("#ligar-gravador").hidden = false;
  $("#ligar-gravador-msg").textContent = "";
  $("#ligar-gravador-titulo").textContent = t("gravador.titulo").replace("{n}", r.canais.length);
  $("#ligar-gravador-nome").value = r.nome || "";
  $("#ligar-gravador-stream").value = "secundario";
  const lista = $("#ligar-gravador-canais");
  delete lista.dataset.pag;
  // Grade: colunas que cabem × poucas linhas; o tela.js pagina e encolhe se faltar altura.
  const cols = Math.max(1, Math.floor((lista.clientWidth || 300) / 178));
  const linhas = innerHeight < 700 ? 2 : innerHeight < 900 ? 3 : 4;
  lista.dataset.porPagina = String(cols * linhas);
  lista.cfN = 0;
  lista.replaceChildren(...r.canais.map((c) => {
    const estado = t(c.responde ? "gravador.responde" : "gravador.nao_responde");
    const canalN = t("gravador.canal_n").replace("{n}", c.canal);
    const chk = el("input", { type: "checkbox", value: String(c.canal), "aria-label": `${canalN}, ${estado}` });
    chk.checked = gravadorAtual.escolhidos.has(c.canal);
    const marcar = (sim) => { chk.checked = sim; if (sim) gravadorAtual.escolhidos.add(c.canal); else gravadorAtual.escolhidos.delete(c.canal); gravadorConta(); };
    chk.onchange = () => marcar(chk.checked);
    // Nome real do canal (título no vídeo do gravador), editável. Vazio = "<gravador> · Canal N".
    const nome = el("input", { class: "canal-nome", maxlength: "60", value: c.nome || "", "data-canal": String(c.canal),
      "aria-label": t("gravador.nome_canal_rotulo").replace("{n}", c.canal), autocomplete: "off", spellcheck: "false" });
    nome.oninput = () => { if (nome.value.trim() && !chk.checked) marcar(true); };
    return el("li", {}, el("div", { class: "canal", title: estado },
      el("label", { class: "canal-esc" },
        chk,
        el("span", { class: "dot " + (c.responde ? "on" : "off"), "aria-hidden": "true" }),
        el("strong", {}, canalN),
        el("span", { class: "tiny" }, estado)),
      nome));
  }));
  gravadorConta();
}

/// Placeholder de cada nome de canal acompanha o nome do gravador.
function gravadorPlaceholders() {
  const base = $("#ligar-gravador-nome").value.trim() || "…";
  $("#ligar-gravador-canais").querySelectorAll(".canal-nome").forEach((i) => { i.placeholder = `${base} · ${t("gravador.canal_n").replace("{n}", i.dataset.canal)}`; });
}

function gravadorMarcarTodos(sim) {
  if (!gravadorAtual) return;
  $("#ligar-gravador-canais").querySelectorAll("input[type=checkbox]").forEach((c) => {
    c.checked = sim;
    if (sim) gravadorAtual.escolhidos.add(Number(c.value)); else gravadorAtual.escolhidos.delete(Number(c.value));
  });
  gravadorConta();
}

async function gravadorAdicionar() {
  if (!gravadorAtual) return;
  const m = $("#ligar-gravador-msg"); const b = $("#ligar-gravador-add");
  if (!gravadorAtual.escolhidos.size) { m.className = "msg err"; m.textContent = t("gravador.escolha"); return; }
  b.disabled = true; m.className = "msg"; m.textContent = t("gravador.adicionando");
  try {
    const nomeDe = (canal) => { const i = $("#ligar-gravador-canais").querySelector(`.canal-nome[data-canal="${canal}"]`); return i ? i.value.trim() : ""; };
    const canais = [...gravadorAtual.escolhidos].sort((a, c) => a - c).map((canal) => ({ canal, nome: nomeDe(canal) || null }));
    const r = await api("/api/gravador/adicionar", {
      token: gravadorAtual.token, nome: $("#ligar-gravador-nome").value.trim(), canais,
      stream: $("#ligar-gravador-stream").value, auto_ip: gravadorAtual.autoIp,
    });
    ultimoLogin = gravadorAtual.login;
    $("#ligar-perguntar").hidden = !ultimoLogin;
    $("#ligar-form").auto_ip.checked = false; $("#ligar-form").nome.value = "";
    m.textContent = "";
    $("#ligar-gravador").hidden = true;
    $("#ligar-ok-texto").textContent = t("gravador.ligadas").replace("{n}", r.cameras.length) + (r.auto_ip && !r.mac_conhecido ? ` ${t("autoip.ligar_depois")}` : "");
    $("#ligar-ok").hidden = false;
    gravadorAtual = null;
    await carregarCameras();
    if (r.cameras.length) abrirCamera(r.cameras[0].id);
    ligarProcurar(false);
  } catch (e) { m.className = "msg err"; m.textContent = e.message; }
  b.disabled = false;
}

function ligarResetar() {
  $("#ligar-gravador").hidden = true;
  gravadorAtual = null;
  $("#ligar-passo2").hidden = true;
  $("#ligar-ok").hidden = true;
  $("#ligar-form").hidden = false;
  ligarAlvo = null;
}

function iniciarLigar() {
  $("#discover").onclick = () => { ligarResetar(); ligarProcurar(); };
  $("#ligar-ffmpeg-btn").onclick = ligarInstalarFfmpeg;
  $("#ligar-form").onsubmit = ligarConectar;
  const aoVivo = $("#nav-aovivo"); if (aoVivo) aoVivo.onclick = () => { location.href = "/ao-vivo"; };
  carregarLogin();
  $("#ligar-outro-login").onclick = () => { usarLoginSalvo = false; mostrarLogin(); $("#ligar-senha").focus(); };
  $("#ligar-esquecer").onclick = async () => {
    try { await api("/api/login-cameras", undefined, "DELETE"); } catch (_) { /* segue */ }
    loginSalvo = null; usarLoginSalvo = false; mostrarLogin();
    $("#ligar-msg").className = "msg"; $("#ligar-msg").textContent = t("login.esquecido");
  };
  $("#ligar-salvar-sim").onclick = async () => {
    if (!ultimoLogin) return;
    try {
      await api("/api/login-cameras", ultimoLogin);
      loginSalvo = ultimoLogin.usuario; usarLoginSalvo = true; ultimoLogin = null;
      $("#ligar-perguntar").hidden = true;
      $("#ligar-ok-texto").textContent += ` ${t("login.salvo_ok")}`;
      mostrarLogin();
    } catch (e) { $("#ligar-ok-texto").textContent = e.message; }
  };
  $("#ligar-salvar-nao").onclick = () => { ultimoLogin = null; $("#ligar-perguntar").hidden = true; };
  $("#ligar-manual").onsubmit = (ev) => { ev.preventDefault(); ligarEscolher(ev.target.ip.value, null); };
  $("#ligar-outra").onclick = ligarResetar;
  $("#ligar-voltar").onclick = ligarResetar;
  $("#ligar-gravador-todos").onclick = () => gravadorMarcarTodos(true);
  $("#ligar-gravador-nenhum").onclick = () => gravadorMarcarTodos(false);
  $("#ligar-gravador-add").onclick = gravadorAdicionar;
  $("#ligar-gravador-nome").oninput = gravadorConta;
  $("#ligar-gravador-voltar").onclick = () => { gravadorAtual = null; $("#ligar-gravador").hidden = true; $("#ligar-form").hidden = false; };
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

function avalRender(r) {
  const lista = $("#sug-lista");
  sugPreview = [];
  sugDesenhar();
  const rodando = !r.terminou;
  $("#sug-andamento").hidden = false;
  $("#sug-cancelar").hidden = false;
  $("#sug-iniciar").hidden = rodando;
  $("#aval-abrir").hidden = rodando;
  $("#aval-linha").hidden = true;
  $("#sug-barra").style.width = `${Math.round((r.progresso || 0) * 100)}%`;
  const nome = r.avaliacao ? r.avaliacao.regra : "";
  $("#sug-status").textContent = r.terminou ? `${t("aval.pronto")} · ${nome}` : `${t("aval.avaliando")} ${nome} · ${t("sug.olhando")} ${r.restante_s} s`;
  const achados = (r.avaliacao && r.avaliacao.achados) || [];
  if (!r.terminou && !achados.length) { lista.replaceChildren(el("li", { class: "tiny" }, t("aval.aguarde"))); return; }
  lista.replaceChildren(...achados.map((a) => el("li", {},
    el("div", { class: "sug-info" },
      el("span", { class: "badge aval-" + a.tom }, t("aval.tom." + a.tom)),
      el("p", { class: "tiny" }, a.texto)))));
}

async function avalIniciar() {
  if (camAberta == null) return;
  const id = Number($("#aval-regra").value);
  if (!id) return;
  clearTimeout(sugTimer);
  $("#sug-lista").replaceChildren();
  try { await api(`/api/cameras/${camAberta}/sugestoes`, { segundos: 60, regra_id: id }); sugAtualizar(); }
  catch (e) { alert(e.message); }
}

function avalAbrir() {
  const sel = $("#aval-regra");
  sel.replaceChildren(...regrasAbertas.map((r) => el("option", { value: r.id }, r.nome)));
  const tem = regrasAbertas.length > 0;
  $("#aval-iniciar").disabled = !tem;
  if (!tem) sel.replaceChildren(el("option", { value: "" }, t("aval.sem_regra")));
  $("#aval-linha").hidden = !$("#aval-linha").hidden;
}

function sugRender(r) {
  if (r.avaliacao) { avalRender(r); return; }
  $("#aval-abrir").hidden = !!(r.iniciado && !r.terminou);
  const lista = $("#sug-lista");
  sugPreview = r.sugestoes || [];
  sugDesenhar();
  const rodando = r.iniciado && !r.terminou;
  $("#sug-andamento").hidden = !r.iniciado;
  $("#sug-cancelar").hidden = !r.iniciado;
  $("#sug-iniciar").hidden = rodando;
  if (!r.iniciado) { lista.replaceChildren(); return; }
  $("#sug-barra").style.width = `${Math.round((r.progresso || 0) * 100)}%`;
  const pessoas = `${r.pessoas} ${r.pessoas === 1 ? t("sug.pessoa") : t("sug.pessoas")}`;
  $("#sug-status").textContent = r.terminou ? `${t("sug.pronto")} · ${pessoas}` : `${t("sug.olhando")} ${r.restante_s} s · ${pessoas}`;
  if (!sugPreview.length) {
    lista.replaceChildren(el("li", { class: "tiny" }, r.terminou ? t("sug.nada") : t("sug.aguarde")));
    return;
  }
  lista.replaceChildren(...sugPreview.map((sg) => {
    const pct = Math.round(sg.confianca * 100);
    const boa = pct >= 70;
    const aceitar = el("button", { class: "btn sm " + (boa ? "primary" : ""), type: "button" }, t("sug.aceitar"));
    aceitar.onclick = () => sugAceitar(sg, pct, aceitar);
    return el("li", {},
      el("div", { class: "sug-info" },
        el("strong", {}, `${sg.nome} · ${t("regra." + sg.forma)}`),
        el("p", { class: "tiny" }, sg.motivo),
        el("span", { class: "badge " + (boa ? "sug-boa" : "sug-fraca") }, `${pct}% · ${t(boa ? "sug.boa" : "sug.fraca")}`)),
      aceitar);
  }));
}

// Para a pesquisa no programa e limpa tudo da tela (traçado, lista, timer).
async function sugParar() {
  clearTimeout(sugTimer); sugTimer = null;
  sugPreview = [];
  sugDesenhar();
  try { await api(`/api/cameras/${camAberta}/sugestoes`, undefined, "DELETE"); } catch (_) { /* segue */ }
  $("#sug-andamento").hidden = true;
  $("#sug-cancelar").hidden = true;
  $("#sug-iniciar").hidden = false;
  $("#sug-iniciar").disabled = false;
  $("#aval-abrir").hidden = false;
  $("#aval-linha").hidden = true;
}

// Uma regra por pesquisa: aceitar cria a regra e encerra a observação.
let sugCriando = false;
async function sugAceitar(sg, pct, botao) {
  if (sugCriando) return;
  const fmt = (k, v) => t(k).replace(/\{(\w+)\}/g, (_, n) => v[n]);
  const ok = await confirmar(fmt("sug.confirma", { nome: sg.nome }), t("sug.criar"),
    pct < 70 ? fmt("sug.confirma_fraca", { pct }) : t("sug.confirma_ok"));
  if (!ok) return;
  sugCriando = true;
  document.querySelectorAll("#sug-lista button").forEach((b) => { b.disabled = true; });
  clearTimeout(sugTimer); sugTimer = null;
  try {
    await api(`/api/cameras/${camAberta}/regras`, {
      nome: sg.nome, forma: sg.forma, pontos: sg.pontos, rotulo_on: "", rotulo_off: "",
      intel: sg.intel || "", classes: sg.classes || [], parametro: sg.parametro || 0,
    });
    await sugParar();
    await carregarRegras();
    $("#sug-lista").replaceChildren(el("li", {}, el("span", { class: "msg ok" }, `${sg.nome} ${t("sug.criada")}`)));
    $("#sug-iniciar").querySelector("span").textContent = t("sug.de_novo");
  } catch (e) {
    document.querySelectorAll("#sug-lista button").forEach((b) => { b.disabled = false; });
    sugAtualizar();
    alert(e.message);
  } finally { sugCriando = false; }
}

async function sugAtualizar() {
  if (camAberta == null) return;
  clearTimeout(sugTimer); sugTimer = null;
  try {
    const r = await api(`/api/cameras/${camAberta}/sugestoes`);
    if (sugCriando) return;
    sugRender(r);
    if (r.iniciado && !r.terminou) sugTimer = setTimeout(sugAtualizar, 2000);
    else { $("#sug-iniciar").disabled = false; }
  } catch (_) { /* segue */ }
}

async function sugIniciar() {
  if (camAberta == null) return;
  clearTimeout(sugTimer);
  $("#sug-iniciar").disabled = true;
  $("#sug-iniciar").querySelector("span").textContent = t("sug.iniciar");
  $("#sug-lista").replaceChildren();
  try { await api(`/api/cameras/${camAberta}/sugestoes`, { segundos: 60 }); sugAtualizar(); }
  catch (e) { alert(e.message); $("#sug-iniciar").disabled = false; }
}

function sugAbrirCamera() {
  clearTimeout(sugTimer); sugTimer = null;
  sugPreview = [];
  sugDesenhar();
  $("#sug-lista").replaceChildren();
  $("#sug-andamento").hidden = true;
  $("#sug-cancelar").hidden = true;
  $("#sug-iniciar").hidden = false;
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
  iniciarTela();

  document.querySelectorAll("[role=tab][data-tab]").forEach((b) => { b.onclick = () => showTab(b.dataset.tab); });
  $("#bench-run").onclick = runBench;
  $("#rtsp-form").onsubmit = buildRtsp;
  $("#rtsp-test").onclick = testRtsp;
  $("#rtsp-copy").onclick = copyRtsp;
  iniciarCameras();
  iniciarLigar();
  iniciarAutoinicio();
  iniciarIntegracoesExtra();
  iniciarFase6();
  iniciarEstudioLocal();
  // rede.js é "defer": pode carregar depois deste arquivo.
  if (typeof iniciarRede === "function") iniciarRede(); else addEventListener("DOMContentLoaded", () => iniciarRede());
  $("#sug-iniciar").onclick = sugIniciar;
  $("#aval-abrir").onclick = avalAbrir;
  $("#aval-iniciar").onclick = avalIniciar;
  const formas = $("#formas-toggle");
  if (formas) {
    formas.checked = mostrarFormas;
    $("#formas-wrap").title = t("formas.dica");
    formas.onchange = () => {
      mostrarFormas = formas.checked;
      try { localStorage.setItem("cf-formas", mostrarFormas ? "1" : "0"); } catch (_) { /* opcional */ }
      desenharSalvas();
    };
  }
  $("#sug-cancelar").onclick = async () => {
    await sugParar();
    $("#sug-lista").replaceChildren(el("li", { class: "tiny" }, t("sug.cancelada")));
  };
  $("#elo-parear").onclick = () => eloAcao(() => api("/api/elo/codigo", {}));
  $("#elo-aceitar").onclick = () => eloAcao(() => api("/api/elo/confirmar", { aceito: true }));
  $("#elo-recusar").onclick = () => eloAcao(() => api("/api/elo/confirmar", { aceito: false }));
  $("#ia-baixar").onclick = async () => { try { await api("/api/ia/baixar", {}); renderIa(); } catch (e) { iaMsg(e.message, "err"); } };
  $("#ia-cancelar").onclick = async () => { try { await api("/api/ia/cancelar", {}); renderIa(); } catch (e) { iaMsg(e.message, "err"); } };
  $("#ia-apagar").onclick = async () => { if (!await confirmar(t("ia.confirma_apagar"))) return; try { await api("/api/ia", undefined, "DELETE"); renderIa(); renderLevels(); } catch (e) { iaMsg(e.message, "err"); } };
  $("#tg-form").onsubmit = tgSalvar;
  $("#tg-testar").onclick = tgTestar;
  $("#tg-remover").onclick = async () => { if (!await confirmar(t("tg.confirma_remover"))) return; try { await api("/api/telegram", undefined, "DELETE"); tgMsg(t("tg.removido"), "ok"); renderTelegram(); } catch (e) { tgMsg(e.message, "err"); } };
  $("#tg-fotos").onchange = async (e) => { try { await api("/api/telegram/fotos", { ligado: e.target.checked }); } catch (err) { tgMsg(err.message, "err"); } };
  // O bot escreve a hora no fuso do técnico: informa o do navegador.
  api("/api/fuso", { minutos: -new Date().getTimezoneOffset() }).catch(() => {});
  $("#ts-copiar-cmd").onclick = async () => { try { await navigator.clipboard.writeText($("#ts-comando").textContent); tsMsg(t("ts.cmd_copiado"), "ok"); } catch (_) { /* sem permissão */ } };
  $("#elo-desparear").onclick = async () => { if (await confirmar(t("elo.confirma_desparear"))) eloAcao(() => api("/api/elo/desparear", {})); };

  const { fabricantes } = await api("/api/rtsp/vendors");
  $("#vendor").replaceChildren(...fabricantes.map((f) =>
    el("option", { value: f.id }, f.tambem.length ? `${f.nome} (mesmo padrão: ${f.tambem.join(", ")})` : f.nome)));

  const { resultado } = await api("/api/bench");
  renderBench(resultado);
  renderSelos();

  let tab = "inicio";
  try { tab = localStorage.getItem("cf-tab") || tab; } catch (_) { /* opcional */ }
  // Sem câmera nenhuma: o programa abre direto em "Ligar câmera".
  try {
    const { cameras } = await api("/api/cameras");
    if (!cameras.length) { tab = "cameras"; irSub("cameras", "ligar", false); }
    else { let s = null; try { s = localStorage.getItem("cf-sub-cameras"); } catch (_) { /* opcional */ } if (!s || s === "aovivo") irSub("cameras", "suas", false); }
  } catch (_) { /* segue */ }
  iniciarManutencao();
  await loadSystem();
  showTab(tab);
  disclaimer();
  setInterval(loadSystem, 15000);
}

main();

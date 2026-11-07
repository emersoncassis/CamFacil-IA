// CamFácil IA — dashboard. JS puro, sem framework, embutido no binário.
"use strict";

let T = {};
const t = (k) => T[k] ?? k;
const $ = (s) => document.querySelector(s);

async function api(path, body) {
  const opt = body === undefined
    ? {}
    : { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) };
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
  ["leve", ["Movimento inteligente", "Cerca e linha virtual", "Saúde das câmeras"]],
  ["medio", ["Pessoas e veículos", "Contagem", "Permanência", "Mapa de calor"]],
  ["pesado", ["Leitura de placas", "Reconhecimento facial"]],
];
const RANK = { leve: 0, medio: 1, pesado: 2 };

function renderLevels() {
  const rows = [];
  for (const [lv, mods] of MODULES) {
    const untested = level == null && lv !== "leve";
    const locked = untested || (level != null && RANK[lv] > RANK[level]);
    const stateText = untested ? t("mod.testar") : locked ? t("mod.bloqueado") : t("mod.disponivel");
    for (const name of mods) {
      const sw = el("label", { class: "switch", title: stateText },
        el("input", { type: "checkbox", disabled: "", "aria-label": name }), el("span"));
      rows.push(el("tr", { class: locked ? "locked" : "" },
        el("td", {}, name),
        el("td", {}, t("nivel." + lv)),
        el("td", {}, el("span", { class: "state" }, locked ? icon("i-lock") : null, stateText)),
        el("td", { class: "right" }, sw)));
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

  document.querySelectorAll("[role=tab]").forEach((b) => { b.onclick = () => showTab(b.dataset.tab); });
  $("#bench-run").onclick = runBench;
  $("#rtsp-form").onsubmit = buildRtsp;
  $("#rtsp-test").onclick = testRtsp;
  $("#rtsp-copy").onclick = copyRtsp;
  $("#discover").onclick = discover;
  $("#found").replaceChildren(emptyRow(t("cam.vazio")));

  const { fabricantes } = await api("/api/rtsp/vendors");
  $("#vendor").replaceChildren(...fabricantes.map((f) =>
    el("option", { value: f.id }, f.tambem.length ? `${f.nome} (mesmo padrão: ${f.tambem.join(", ")})` : f.nome)));

  const { resultado } = await api("/api/bench");
  renderBench(resultado);

  let tab = "inicio";
  try { tab = localStorage.getItem("cf-tab") || tab; } catch (_) { /* opcional */ }
  await loadSystem();
  showTab(tab);
  disclaimer();
  setInterval(loadSystem, 15000);
}

main();

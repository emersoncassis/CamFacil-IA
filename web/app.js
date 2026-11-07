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
  for (const c of children) e.append(c);
  return e;
}

function msg(text, kind) {
  const m = $("#rtsp-msg");
  m.textContent = text;
  m.className = "msg " + (kind || "");
}

// ---------- Abas ----------
function showTab(id) {
  document.querySelectorAll("[role=tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === id)));
  document.querySelectorAll("[role=tabpanel]").forEach((p) => { p.hidden = p.id !== id; });
  try { localStorage.setItem("cf-tab", id); } catch (_) { /* opcional */ }
  if (id === "modulos") renderLevels();
}

// ---------- Início ----------
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
      ["sys.ram", `${s.ram_used_mb} / ${s.ram_total_mb} MB`],
      ["sys.self", s.self_ram_mb != null ? s.self_ram_mb + " MB" : "—"],
      ["sys.versao", s.version],
    ];
    const dl = $("#sysinfo");
    dl.replaceChildren(...rows.flatMap(([k, v]) => [el("dt", {}, t(k)), el("dd", {}, String(v))]));
    $("#topstatus").textContent = `CPU ${s.cpu_usage.toFixed(0)}% · RAM ${s.ram_used_mb} MB`;
  } catch (_) {
    $("#topstatus").textContent = t("erro.rede");
  }
}

function renderBench(r) {
  const box = $("#bench-result");
  if (!r) { box.replaceChildren(el("p", { class: "muted" }, t("inicio.nunca"))); return; }
  const m = (v, k) => el("div", { class: "metric" }, el("strong", {}, String(v)), el("span", {}, t(k)));
  box.replaceChildren(
    m(t("nivel." + r.nivel), "inicio.nivel"),
    m(r.fps_estimado, "inicio.fps"),
    m(r.cameras_medio, "inicio.cams"),
  );
}

async function runBench() {
  const b = $("#bench-run");
  b.disabled = true; b.textContent = t("inicio.testando");
  try {
    const { resultado } = await api("/api/bench", {});
    level = resultado.nivel;
    renderBench(resultado);
  } catch (e) { renderBench(null); }
  b.disabled = false; b.textContent = t("inicio.teste_btn");
}

// ---------- Módulos ----------
const LEVELS = [
  { id: "leve", cor: "var(--cf-green)", mods: ["Movimento inteligente", "Cerca e linha virtual", "Saúde das câmeras"] },
  { id: "medio", cor: "var(--cf-warn)", mods: ["Pessoas e veículos", "Contagem", "Permanência", "Mapa de calor"] },
  { id: "pesado", cor: "var(--cf-danger)", mods: ["Leitura de placas", "Reconhecimento facial"] },
];
const RANK = { leve: 0, medio: 1, pesado: 2 };

function renderLevels() {
  const wrap = $("#levels");
  wrap.replaceChildren(...LEVELS.map((l) => {
    const locked = level == null ? l.id !== "leve" : RANK[l.id] > RANK[level];
    const note = level == null && l.id !== "leve" ? t("nivel.faca_teste") : locked ? t("nivel.bloqueado") : t("nivel.em_breve");
    return el("article", { class: "card level" + (locked ? " locked" : "") },
      el("h2", {}, el("span", { class: "pip", style: `background:${l.cor}` }), t("nivel." + l.id)),
      el("ul", {}, ...l.mods.map((m) => el("li", {}, m))),
      el("p", { class: locked ? "lock" : "muted" }, note));
  }));
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
  if (!d.ip) { msg(t("cam.ip"), "err"); return; }
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

async function discover() {
  const b = $("#discover"); const list = $("#found");
  b.disabled = true; b.textContent = t("cam.buscando");
  try {
    const { cameras } = await api("/api/discover");
    if (!cameras.length) list.replaceChildren(el("li", {}, el("small", {}, t("cam.nenhuma"))));
    else list.replaceChildren(...cameras.map((c) => {
      const use = el("button", { class: "btn small", type: "button" }, t("cam.usar"));
      use.onclick = () => {
        const f = $("#rtsp-form");
        f.ip.value = c.ip;
        const v = (c.fabricante || "").toLowerCase();
        if ([...f.fabricante.options].some((o) => o.value === v)) f.fabricante.value = v;
        f.senha.focus();
      };
      const desc = [c.fabricante, c.modelo].filter(Boolean).join(" · ") || c.nome || "";
      return el("li", {},
        el("div", {}, el("div", { class: "mono" }, c.ip), el("small", {}, [desc, c.mac].filter(Boolean).join(" · "))),
        use);
    }));
  } catch (e) { list.replaceChildren(el("li", {}, el("small", {}, e.message))); }
  b.disabled = false; b.textContent = t("cam.busca_btn");
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

  const { fabricantes } = await api("/api/rtsp/vendors");
  $("#vendor").replaceChildren(...fabricantes.map((f) =>
    el("option", { value: f.id }, f.tambem.length ? `${f.nome} (padrão ${f.tambem.join(", ")})` : f.nome)));

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

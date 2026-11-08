// CamFácil IA — tela Manutenção (fase 5). Usa os ajudantes do app.js
// (t, $, el, api), carregado logo depois dele.
"use strict";

function manMsg(id, texto, tipo) { const m = $(id); m.textContent = texto || ""; m.className = "msg " + (tipo || ""); }

// ---------- Modo TV ----------
let tvInfo = null;
let tvOrdem = []; // ids na ordem do mosaico

function tvRenderCameras() {
  const lista = $("#tv-cameras");
  if (!tvInfo || !tvInfo.cameras.length) {
    lista.replaceChildren(el("li", {}, el("p", {}, t("tv.sem_cameras_cfg"))));
    return;
  }
  const todas = tvInfo.cameras;
  // Ordem: as escolhidas primeiro, depois as não escolhidas.
  const ordenadas = [...tvOrdem.map((id) => todas.find((c) => c.id === id)).filter(Boolean), ...todas.filter((c) => !tvOrdem.includes(c.id))];
  lista.replaceChildren(...ordenadas.map((c, i) => {
    const marcada = tvOrdem.includes(c.id);
    const chk = el("input", { type: "checkbox", "aria-label": c.nome });
    chk.checked = marcada;
    chk.onchange = () => { if (chk.checked) tvOrdem.push(c.id); else tvOrdem = tvOrdem.filter((x) => x !== c.id); tvRenderCameras(); };
    const sobe = el("button", { class: "btn ghost sm", type: "button", title: t("tv.subir"), disabled: "" }, "↑");
    const desce = el("button", { class: "btn ghost sm", type: "button", title: t("tv.descer"), disabled: "" }, "↓");
    if (marcada) {
      const pos = tvOrdem.indexOf(c.id);
      if (pos > 0) { sobe.removeAttribute("disabled"); sobe.onclick = () => { [tvOrdem[pos - 1], tvOrdem[pos]] = [tvOrdem[pos], tvOrdem[pos - 1]]; tvRenderCameras(); }; }
      if (pos < tvOrdem.length - 1) { desce.removeAttribute("disabled"); desce.onclick = () => { [tvOrdem[pos + 1], tvOrdem[pos]] = [tvOrdem[pos], tvOrdem[pos + 1]]; tvRenderCameras(); }; }
    }
    return el("li", {},
      el("div", {}, el("label", { class: "som-regra" }, el("span", { class: "switch" }, chk, el("span")), el("strong", {}, c.nome),
        el("span", { class: "tiny" }, marcada ? `${t("tv.posicao")} ${i + 1}` : t("tv.fora_do_mosaico"))),
        el("p", {}, `${t("fonte." + c.fonte)} · ${c.online ? t("estado.online") : t("estado.conectando")}`)),
      el("span", { class: "ordem" }, sobe, desce));
  }));
}

function tvEndereco() {
  if (!tvInfo) return "";
  const ip = tvInfo.lan && tvInfo.ip_rede ? tvInfo.ip_rede : location.hostname;
  return `http://${ip}:${tvInfo.porta}/tv`;
}

async function tvCarregar() {
  try {
    tvInfo = await api("/api/tv");
    const c = tvInfo.config;
    tvOrdem = c.cameras.length ? c.cameras.filter((id) => tvInfo.cameras.some((k) => k.id === id)) : tvInfo.cameras.map((k) => k.id);
    tvRenderCameras();
    $("#tv-whatsapp").value = c.whatsapp || "";
    $("#tv-chamada").value = c.chamada || "";
    $("#tv-som").checked = !!c.som;
    $("#tv-piscar").checked = !!c.piscar;
    $("#tv-relogio").checked = !!c.relogio;
    $("#tv-endereco").textContent = tvEndereco();
    $("#tv-endereco-dica").textContent = tvInfo.lan ? t("tv.endereco_lan") : t("tv.endereco_local");
    tvLogoRender();
    $("#tv-passos").replaceChildren(...(T["tv.passos"] || []).map((p) => el("li", {}, p.replace("{url}", tvEndereco()))));
  } catch (e) { manMsg("#tv-msg", e.message, "err"); }
}

function tvLogoRender() {
  const img = $("#tv-logo-img");
  const tem = tvInfo && tvInfo.logo;
  img.hidden = !tem; $("#tv-logo-vazio").hidden = tem; $("#tv-logo-remover").hidden = !tem;
  if (tem) img.src = "/api/tv/logo.png?" + Date.now();
}

async function tvSalvar() {
  const b = $("#tv-salvar"); b.disabled = true;
  try {
    tvInfo = await api("/api/tv", {
      cameras: tvOrdem, som: $("#tv-som").checked, piscar: $("#tv-piscar").checked, relogio: $("#tv-relogio").checked,
      whatsapp: $("#tv-whatsapp").value, chamada: $("#tv-chamada").value,
    });
    $("#tv-whatsapp").value = tvInfo.config.whatsapp;
    manMsg("#tv-msg", tvInfo.whatsapp_link ? `${t("tv.salvo")} · ${tvInfo.whatsapp_link}` : t("tv.salvo"), "ok");
  } catch (e) { manMsg("#tv-msg", e.message, "err"); }
  b.disabled = false;
}

async function tvEnviarLogo(file) {
  if (!file) return;
  if (file.size > 200 * 1024) { manMsg("#tv-msg", t("tv.logo_grande"), "err"); return; }
  try {
    const r = await fetch("/api/tv/logo", { method: "POST", headers: { "Content-Type": "image/png" }, body: file });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.erro || r.statusText);
    tvInfo.logo = true; tvLogoRender(); manMsg("#tv-msg", t("tv.logo_ok"), "ok");
  } catch (e) { manMsg("#tv-msg", e.message, "err"); }
}

function tvIniciar() {
  $("#tv-salvar").onclick = tvSalvar;
  $("#tv-copiar").onclick = async () => { try { await navigator.clipboard.writeText(tvEndereco()); manMsg("#tv-msg", t("tv.copiado"), "ok"); } catch (_) { /* sem permissão */ } };
  $("#tv-logo-file").onchange = (e) => { tvEnviarLogo(e.target.files[0]); e.target.value = ""; };
  $("#tv-logo-remover").onclick = async () => { try { await api("/api/tv/logo", undefined, "DELETE"); tvInfo.logo = false; tvLogoRender(); } catch (e) { manMsg("#tv-msg", e.message, "err"); } };
}

// ---------- Máquina (monitor de hardware) ----------
let hwTimer = null;
let hwHoras = 24;

function hwFmtUptime(s) {
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d ? `${d} ${t("tempo.d")} ${h} ${t("tempo.h")}` : h ? `${h} ${t("tempo.h")} ${m} ${t("tempo.min")}` : `${m} ${t("tempo.min")}`;
}

function hwRenderAtual(r) {
  const a = r.atual;
  const dl = $("#hw-atual");
  if (!a) { dl.replaceChildren(el("dt", {}, t("hw.situacao")), el("dd", {}, t("hw.aguardando"))); return; }
  const linhas = [
    ["hw.cpu", `${a.cpu.toFixed(0)} %`, a.cpu > r.limites.cpu],
    ["hw.ram", `${a.ram_pct.toFixed(0) } % · ${a.ram_mb} ${t("hw.de")} ${a.ram_total_mb} MB`, a.ram_pct > r.limites.ram],
    ["hw.temp", a.temperatura == null ? t("hw.sem_sensor") : `${a.temperatura.toFixed(0)} °C`, a.temperatura != null && a.temperatura > r.limites.temperatura],
    ["hw.disco", a.disco_livre_mb == null ? "—" : a.disco_livre_mb >= 10240 ? `${(a.disco_livre_mb / 1024).toFixed(1)} GB ${t("hw.livres")}` : `${a.disco_livre_mb} MB ${t("hw.livres")}`, a.disco_livre_mb != null && a.disco_livre_mb < r.limites.disco_mb],
    ["hw.uptime", hwFmtUptime(a.uptime_s), false],
    ["hw.fps", Object.keys(a.fps).length ? Object.entries(a.fps).map(([id, f]) => `${(tvInfo && tvInfo.cameras.find((c) => String(c.id) === id) || { nome: `#${id}` }).nome}: ${f.toFixed(1)}`).join(" · ") : "—", false],
    ["hw.amostras", `${r.amostras} (${t("hw.a_cada")} ${r.intervalo_s} s)`, false],
  ];
  dl.replaceChildren(...linhas.flatMap(([k, v, alerta]) => [el("dt", {}, t(k)), el("dd", { class: alerta ? "alerta" : "" }, v)]));
}

/// Gráfico de linhas em SVG, sem biblioteca. CPU e RAM em %, temperatura em °C (mesma escala 0–100).
function hwRenderGrafico(amostras, horas, limites) {
  const W = 960, H = 260, L = 36, R = 12, T0 = 12, B = 28;
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  const mk = (tag, attrs, txt) => { const e = document.createElementNS(ns, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); if (txt != null) e.textContent = txt; return e; };
  const agora = Math.floor(Date.now() / 1000), ini = agora - horas * 3600;
  const x = (q) => L + ((q - ini) / (agora - ini)) * (W - L - R);
  const y = (v) => T0 + (1 - Math.min(100, Math.max(0, v)) / 100) * (H - T0 - B);
  for (const v of [0, 25, 50, 75, 100]) {
    svg.append(mk("line", { class: "grade", x1: L, x2: W - R, y1: y(v), y2: y(v) }));
    svg.append(mk("text", { class: "eixo", x: L - 6, y: y(v) + 4, "text-anchor": "end" }, String(v)));
  }
  const passos = horas <= 24 ? 6 : 7;
  for (let i = 0; i <= passos; i++) {
    const q = ini + (i / passos) * (agora - ini);
    const d = new Date(q * 1000);
    const rot = horas <= 24 ? d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }) : d.toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" });
    svg.append(mk("text", { class: "eixo", x: x(q), y: H - 8, "text-anchor": i === 0 ? "start" : i === passos ? "end" : "middle" }, rot));
  }
  svg.append(mk("line", { class: "limite", x1: L, x2: W - R, y1: y(limites.cpu), y2: y(limites.cpu) }));
  if (!amostras.length) {
    svg.append(mk("text", { class: "vazio", x: W / 2, y: H / 2, "text-anchor": "middle" }, t("hw.sem_dados")));
  } else {
    const serie = (cls, f) => {
      let d = "", aberto = false, anterior = null;
      for (const a of amostras) {
        const v = f(a);
        if (v == null) { aberto = false; continue; }
        // Buraco maior que 3 intervalos (máquina desligada): levanta a caneta.
        if (anterior != null && a.quando - anterior > (a.agregada ? 3 * 3600 : 3 * 60) * 1.5) aberto = false;
        d += `${aberto ? "L" : "M"}${x(a.quando).toFixed(1)} ${y(v).toFixed(1)} `;
        aberto = true; anterior = a.quando;
      }
      if (d) svg.append(mk("path", { class: "serie " + cls, d }));
    };
    serie("ram", (a) => a.ram_pct);
    serie("temp", (a) => a.temperatura);
    serie("cpu", (a) => a.cpu);
  }
  const box = $("#hw-grafico");
  box.setAttribute("aria-label", t("hw.grafico_alt"));
  box.replaceChildren(svg);
  $("#hw-legenda").replaceChildren(
    ...[["cpu", "hw.cpu"], ["ram", "hw.ram"], ["temp", "hw.temp"]].map(([c, k]) => el("span", { class: "hw-legenda-item" }, el("i", { class: c }), t(k))),
    el("span", { class: "hw-legenda-item" }, el("i", { class: "lim" }), `${t("hw.limite_cpu")} ${limites.cpu} %`));
}

async function hwCarregar() {
  if ($("#manutencao").hidden) return;
  try {
    const r = await api("/api/hardware");
    hwRenderAtual(r);
    const log = await api(`/api/hardware/log?horas=${hwHoras}`);
    hwRenderGrafico(log.amostras, log.horas, r.limites);
    const agora = Math.floor(Date.now() / 1000);
    $("#hw-eventos").replaceChildren(...(r.eventos.length
      ? r.eventos.map((e) => el("li", {}, el("div", {}, el("p", {}, e.texto)),
        el("span", { class: "tiny" }, `${new Date(e.quando * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" })} · ${tempoRelativo(agora - e.quando)} ${t("tempo.atras")}`)))
      : [el("li", {}, el("p", {}, t("hw.sem_alertas")))]));
    manMsg("#hw-msg", "");
  } catch (e) { manMsg("#hw-msg", e.message, "err"); }
}

function hwAtivar(ativo) {
  clearInterval(hwTimer); hwTimer = null;
  if (ativo) { hwCarregar(); hwTimer = setInterval(hwCarregar, 30000); }
}

function hwIniciar() {
  document.querySelectorAll("#hw-periodo input").forEach((i) => { i.onchange = () => { hwHoras = Number(i.value); hwCarregar(); }; });
}

// ---------- Relatório em PDF ----------
function relTamanho(b) { return b >= 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`; }

async function relCarregar() {
  try {
    const r = await api("/api/relatorio");
    if (!$("#rel-cliente").value) $("#rel-cliente").value = r.cliente || "";
    if (!$("#rel-tecnico").value) $("#rel-tecnico").value = r.tecnico || "";
    $("#rel-pasta").textContent = `${t("rel.pasta")} ${r.pasta}`;
    const lista = $("#rel-lista");
    if (!r.relatorios.length) { lista.replaceChildren(el("li", {}, el("p", {}, t("rel.nenhum")))); return; }
    lista.replaceChildren(...r.relatorios.map((x) => {
      const url = `/api/relatorio/${encodeURIComponent(x.arquivo)}`;
      const tg = el("button", { class: "btn ghost sm", type: "button" }, t("rel.telegram"));
      tg.onclick = async () => {
        tg.disabled = true;
        try { const d = await api(`${url}/telegram`, {}); manMsg("#rel-msg", t("rel.enviado").replace("{n}", d.enviados), "ok"); }
        catch (e) { manMsg("#rel-msg", e.message, "err"); }
        tg.disabled = false;
      };
      const apagar = el("button", { class: "btn ghost sm", type: "button" }, t("rel.apagar"));
      apagar.onclick = async () => {
        if (!confirm(t("rel.apagar_confirma"))) return;
        try { await api(url, undefined, "DELETE"); relCarregar(); } catch (e) { manMsg("#rel-msg", e.message, "err"); }
      };
      return el("li", {},
        el("div", {}, el("p", {}, el("strong", {}, x.arquivo)),
          el("span", { class: "tiny" }, `${new Date(x.quando * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" })} · ${relTamanho(x.bytes)}`)),
        el("span", { class: "actions" }, el("a", { class: "btn sm", href: url, download: x.arquivo }, t("rel.baixar")), tg, apagar));
    }));
  } catch (e) { manMsg("#rel-msg", e.message, "err"); }
}

async function relGerar() {
  const b = $("#rel-gerar"); b.disabled = true;
  manMsg("#rel-msg", t("rel.gerando"));
  try {
    const r = await api("/api/relatorio", { cliente: $("#rel-cliente").value, tecnico: $("#rel-tecnico").value, observacoes: $("#rel-obs").value });
    manMsg("#rel-msg", `${t("rel.gerado")} · ${r.arquivo} · ${relTamanho(r.bytes)}`, "ok");
    relCarregar();
  } catch (e) { manMsg("#rel-msg", e.message, "err"); }
  b.disabled = false;
}

function relIniciar() { $("#rel-gerar").onclick = relGerar; }

// ---------- Backup cifrado ----------
function bkpSenhaNova() {
  const s = $("#bkp-senha").value;
  if (s.length < 8) { manMsg("#bkp-msg", t("bkp.senha_curta"), "err"); return null; }
  if (s !== $("#bkp-senha2").value) { manMsg("#bkp-msg", t("bkp.senhas_diferentes"), "err"); return null; }
  return s;
}

async function bkpBaixar() {
  const senha = bkpSenhaNova(); if (!senha) return;
  const b = $("#bkp-baixar"); b.disabled = true;
  manMsg("#bkp-msg", t("bkp.gerando"));
  try {
    const r = await fetch("/api/backup", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ senha }) });
    if (!r.ok) { const d = await r.json().catch(() => ({})); throw new Error(d.erro || r.statusText); }
    const nome = (/filename="([^"]+)"/.exec(r.headers.get("Content-Disposition") || "") || [])[1] || "camfacil-backup.cfbkp";
    const url = URL.createObjectURL(await r.blob());
    const a = el("a", { href: url, download: nome }); document.body.append(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
    manMsg("#bkp-msg", t("bkp.baixado").replace("{arquivo}", nome), "ok");
  } catch (e) { manMsg("#bkp-msg", e.message, "err"); }
  b.disabled = false;
}

async function bkpTelegram() {
  const senha = bkpSenhaNova(); if (!senha) return;
  const b = $("#bkp-telegram"); b.disabled = true;
  manMsg("#bkp-msg", t("bkp.gerando"));
  try { const d = await api("/api/backup/telegram", { senha }); manMsg("#bkp-msg", t("bkp.enviado").replace("{n}", d.enviados), "ok"); }
  catch (e) { manMsg("#bkp-msg", e.message, "err"); }
  b.disabled = false;
}

async function bkpRestaurar() {
  const f = $("#bkp-arquivo").files[0];
  if (!f) { manMsg("#bkp-msg", t("bkp.sem_arquivo"), "err"); return; }
  if (!confirm(t("bkp.confirma"))) return;
  const senha = new TextEncoder().encode($("#bkp-senha-imp").value);
  const cab = new Uint8Array(2 + senha.length);
  cab[0] = senha.length >> 8; cab[1] = senha.length & 255; cab.set(senha, 2);
  const b = $("#bkp-restaurar"); b.disabled = true;
  manMsg("#bkp-msg", t("bkp.restaurando"));
  try {
    const r = await fetch("/api/backup/importar", { method: "POST", headers: { "Content-Type": "application/octet-stream" }, body: new Blob([cab, f]) });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.erro || r.statusText);
    const quando = new Date(d.criado_em * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
    manMsg("#bkp-msg", t("bkp.restaurado").replace("{cameras}", d.cameras).replace("{regras}", d.regras).replace("{contatos}", d.contatos).replace("{quando}", quando), "ok");
    $("#bkp-senha-imp").value = ""; $("#bkp-arquivo").value = "";
    tvCarregar();
  } catch (e) { manMsg("#bkp-msg", e.message, "err"); }
  b.disabled = false;
}

function bkpIniciar() {
  $("#bkp-baixar").onclick = bkpBaixar;
  $("#bkp-telegram").onclick = bkpTelegram;
  $("#bkp-restaurar").onclick = bkpRestaurar;
}

// ---------- Entrada da tela ----------
let manutencaoCarregada = false;
function manutencaoAtivar(ativo) {
  hwAtivar(ativo);
  if (!ativo) return;
  tvCarregar();
  relCarregar();
  manutencaoCarregada = true;
}

function iniciarManutencao() {
  tvIniciar();
  hwIniciar();
  relIniciar();
  bkpIniciar();
}

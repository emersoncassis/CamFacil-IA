// CamFácil IA — site do domínio. JS puro.
"use strict";

const $ = (s) => document.querySelector(s);

async function api(caminho, metodo = "GET", corpo) {
  const opt = { method: metodo, headers: {}, credentials: "same-origin" };
  if (corpo !== undefined) { opt.headers["Content-Type"] = "application/json"; opt.body = JSON.stringify(corpo); }
  const r = await fetch(caminho, opt);
  const dados = r.status === 204 ? {} : await r.json().catch(() => ({}));
  if (!r.ok) { const e = new Error(dados.erro || r.statusText); e.status = r.status; throw e; }
  return dados;
}

function el(tag, attrs = {}, ...filhos) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) { if (k === "class") e.className = v; else e.setAttribute(k, v); }
  for (const f of filhos) if (f != null) e.append(f);
  return e;
}

const data = (s) => (s ? new Date(s).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) : "—");

// ---------- Chave do dispositivo (WebCrypto, não exportável) ----------
function idb() {
  return new Promise((ok, falha) => {
    const r = indexedDB.open("cfia", 1);
    r.onupgradeneeded = () => r.result.createObjectStore("chaves");
    r.onsuccess = () => ok(r.result);
    r.onerror = () => falha(r.error);
  });
}
async function idbGet(k) {
  const db = await idb();
  return new Promise((ok) => { const r = db.transaction("chaves").objectStore("chaves").get(k); r.onsuccess = () => ok(r.result); r.onerror = () => ok(undefined); });
}
async function idbSet(k, v) {
  const db = await idb();
  return new Promise((ok, falha) => { const t = db.transaction("chaves", "readwrite"); t.objectStore("chaves").put(v, k); t.oncomplete = ok; t.onerror = () => falha(t.error); });
}

function apelidoDispositivo() {
  const ua = navigator.userAgent;
  const so = /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iPhone/iPad" : /Windows/.test(ua) ? "Windows" : /Mac/.test(ua) ? "Mac" : /Linux/.test(ua) ? "Linux" : "Dispositivo";
  const nav = /Edg\//.test(ua) ? "Edge" : /Chrome\//.test(ua) ? "Chrome" : /Firefox\//.test(ua) ? "Firefox" : /Safari\//.test(ua) ? "Safari" : "navegador";
  return `${nav} no ${so}`;
}

let eu = null;

async function garantirDispositivo(forcarNovo = false) {
  const chave = `dispositivo:${eu.id}`;
  const salvo = forcarNovo ? null : await idbGet(chave);
  if (salvo && salvo.id) return salvo;
  const par = await crypto.subtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, false, ["sign", "verify"]);
  const spki = new Uint8Array(await crypto.subtle.exportKey("spki", par.publicKey));
  const b64 = btoa(String.fromCharCode(...spki));
  const { id } = await api("/api/dispositivos", "POST", { chave_publica: b64, apelido: apelidoDispositivo() });
  const reg = { id, par };
  await idbSet(chave, reg);
  return reg;
}

// ---------- Navegação ----------
function mostrar(id) {
  document.querySelectorAll("[role=tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === id)));
  document.querySelectorAll("main [role=tabpanel]").forEach((p) => { p.hidden = p.id !== id; });
  if (id === "agentes") carregarAgentes();
  if (id === "dispositivos") carregarDispositivos();
}

// ---------- Agentes ----------
async function carregarAgentes() {
  const corpo = $("#lista-agentes");
  try {
    const { agentes } = await api("/api/agentes");
    if (!agentes.length) {
      corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "5" }, "Nenhum agente pareado ainda. Clique em Parear agente para ver como.")));
      return;
    }
    corpo.replaceChildren(...agentes.map((a) => {
      const campo = el("input", { class: "cell-edit", value: a.apelido_cliente, placeholder: "Nome do cliente", "aria-label": "Nome do cliente", maxlength: "60" });
      campo.addEventListener("change", async () => {
        try { await api(`/api/agentes/${a.id}`, "PATCH", { apelido_cliente: campo.value }); } catch (e) { campo.value = a.apelido_cliente; alert(e.message); }
      });
      return el("tr", { "data-agente": a.id },
        el("td", {}, campo),
        el("td", {}, el("span", { class: "state" }, el("span", { class: "dot " + (a.online ? "on" : "") }), a.online ? "On-line" : "Desconectado")),
        el("td", { class: "mono" }, a.impressao),
        el("td", { class: "mono" }, a.versao || "—"),
        el("td", { class: "right" }, data(a.pareado_em)));
    }));
  } catch (e) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "5" }, e.message)));
  }
}

// ---------- Dispositivos ----------
async function carregarDispositivos() {
  const corpo = $("#lista-dispositivos");
  const atual = await idbGet(`dispositivo:${eu.id}`);
  const { dispositivos } = await api("/api/dispositivos");
  const ativos = dispositivos.filter((d) => !d.revogado);
  if (!ativos.length) { corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "4" }, "Nenhum dispositivo ativo."))); return; }
  corpo.replaceChildren(...ativos.map((d) => {
    const btn = el("button", { class: "btn sm", type: "button" }, "Revogar");
    btn.onclick = async () => {
      if (!confirm(`Revogar "${d.apelido || "dispositivo"}"? Os agentes deixam de aceitar comandos dele.`)) return;
      await api(`/api/dispositivos/${d.id}`, "DELETE");
      carregarDispositivos();
    };
    const nome = d.apelido || "Dispositivo";
    return el("tr", {},
      el("td", {}, atual && atual.id === d.id ? `${nome} (este)` : nome),
      el("td", { class: "mono" }, d.impressao),
      el("td", {}, data(d.criado_em)),
      el("td", { class: "right" }, btn));
  }));
}

// ---------- Pareamento ----------
function codigoDaUrl() {
  const p = new URLSearchParams(location.hash.slice(1));
  return { codigo: p.get("c"), impressao: p.get("f") };
}

function msgPar(texto, tipo) { const m = $("#par-msg"); m.textContent = texto; m.className = "msg " + (tipo || ""); }

async function telaParear() {
  const { codigo, impressao } = codigoDaUrl();
  mostrar("parear");
  $("#par-impressao").textContent = impressao || "—";
  $("#par-conta").textContent = eu.email;
  if (!codigo) { msgPar("Link sem código. Gere um novo QR na tela do cliente.", "err"); $("#par-confirmar").disabled = true; return; }
  $("#par-cancelar").onclick = () => { history.replaceState(null, "", "/"); mostrar("agentes"); };
  $("#par-confirmar").onclick = async () => {
    const b = $("#par-confirmar");
    b.disabled = true;
    try {
      let disp = await garantirDispositivo();
      let r;
      try {
        r = await api("/api/parear", "POST", { codigo, dispositivo_id: disp.id });
      } catch (e) {
        if (/dispositivo/.test(e.message)) { disp = await garantirDispositivo(true); r = await api("/api/parear", "POST", { codigo, dispositivo_id: disp.id }); } else throw e;
      }
      if (impressao && r.impressao_agente !== impressao) {
        msgPar("Atenção: a impressão do agente não confere com a do QR. Recuse na tela do cliente.", "err");
        return;
      }
      msgPar("Pedido enviado. Agora clique em Confirmar na tela do cliente.", "ok");
    } catch (e) {
      msgPar(e.message, "err");
      b.disabled = false;
    }
  };
}

// ---------- Tempo real ----------
function conectarPainel() {
  const ws = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws/painel`);
  ws.onopen = () => { $("#live-dot").className = "dot on"; };
  ws.onclose = () => { $("#live-dot").className = "dot"; setTimeout(conectarPainel, 5000); };
  ws.onmessage = (ev) => {
    let m; try { m = JSON.parse(ev.data); } catch (_) { return; }
    if (m.tipo === "pareado") {
      msgPar("Agente pareado com sucesso.", "ok");
      setTimeout(() => { history.replaceState(null, "", "/"); mostrar("agentes"); }, 1200);
    } else if (m.tipo === "pareamento_recusado") {
      msgPar("O pareamento foi recusado na tela do cliente.", "err");
    } else if (["agente_online", "agente_offline", "despareado"].includes(m.tipo) && !$("#agentes").hidden) {
      carregarAgentes();
    }
  };
}

// ---------- Início ----------
async function main() {
  const erro = new URLSearchParams(location.search).get("erro");
  try {
    eu = await api("/api/eu");
  } catch (_) {
    if (location.pathname === "/parear") sessionStorage.setItem("cfia-voltar", location.pathname + location.hash);
    $("#entrar").hidden = false;
    if (erro) $("#entrar-erro").textContent = "Login cancelado. Tente de novo.";
    return;
  }
  const voltar = sessionStorage.getItem("cfia-voltar");
  if (voltar) { sessionStorage.removeItem("cfia-voltar"); history.replaceState(null, "", voltar); }

  $("#app").hidden = false;
  $("#eu-email").textContent = eu.email;
  document.querySelectorAll("[role=tab]").forEach((b) => { b.onclick = () => { history.replaceState(null, "", "/"); mostrar(b.dataset.tab); }; });
  $("#como-parear").onclick = () => { $("#passos").hidden = !$("#passos").hidden; };
  $("#sair").onclick = async () => { await api("/api/auth/sair", "POST"); location.href = "/"; };

  garantirDispositivo().catch(() => {});
  conectarPainel();
  if (location.pathname === "/parear") telaParear(); else mostrar("agentes");
}

main();

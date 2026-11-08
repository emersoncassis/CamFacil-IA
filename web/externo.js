// CamFácil IA — acesso de fora com HTTPS: endereço HTTPS do MCP pelo
// Tailscale (Serve/Funnel) e túnel temporário do ngrok (painel ou só MCP).
// Usa api, t, $, el, confirmar, irSub e ieMsg dos outros scripts. JS puro.
"use strict";

let extMcp = null;      // último /api/mcp
let extHttps = null;    // último /api/mcp/https
let extNgrok = null;    // último /api/ngrok
let extTimer = null;
let extOcupado = false;

function extDuracao(s) {
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
  if (h >= 1) return t("ngrok.ha_h").replace("{h}", h).replace("{m}", m);
  return t("ngrok.ha_m").replace("{m}", Math.max(1, m));
}

async function extCopiar(sel, msgSel) {
  const e = $(sel);
  if (!e) return;
  try { await navigator.clipboard.writeText(e.textContent.trim()); ieMsg(msgSel, t("ext.copiado"), "ok"); }
  catch (_) { /* sem permissão de área de transferência */ }
}

// ---------- MCP por HTTPS ----------
function mcphPasso(texto, link) {
  const li = el("li", {}, texto);
  if (link) { li.append(" "); li.append(el("a", { href: link, target: "_blank", rel: "noopener" }, link.replace(/^https:\/\//, ""))); }
  return li;
}

function mcphDesenhar() {
  const h = extHttps, m = extMcp, n = extNgrok;
  if (!m || !m.ligado) return;
  const passos = [];
  let estado = "", dot = "dot", ativo = null;
  const ngrokMcp = n && n.ligado && n.alvo === "mcp" ? n : null;
  const podeTs = h && !h.android && h.instalado && h.versao_ok && h.logado && h.https_ok;
  if (h && h.publicado) {
    ativo = { url: h.publicado.url, modo: h.publicado.modo };
    estado = t(h.publicado.modo === "funnel" ? "mcph.no_ar_funnel" : "mcph.no_ar_serve");
    dot = "dot on";
  } else if (ngrokMcp) {
    ativo = { url: ngrokMcp.url, modo: "ngrok" };
    estado = t("mcph.no_ar_ngrok");
    dot = "dot on";
  } else if (!h) {
    estado = t("mcph.verificando");
  } else if (h.android) {
    estado = t("mcph.android");
  } else if (!h.instalado) {
    estado = t("mcph.sem_tailscale");
    passos.push(mcphPasso(t("mcph.passo_instalar")));
  } else if (!h.versao_ok) {
    estado = t("mcph.antigo").replace("{v}", h.versao || "?").replace("{min}", h.versao_minima);
  } else if (h.erro || !h.logado) {
    estado = h.erro || t("ts.nao_logado");
    dot = "dot off";
  } else if (!h.https_ok) {
    estado = t("mcph.sem_https");
    passos.push(mcphPasso(t("mcph.passo_https"), h.links.dns));
    passos.push(mcphPasso(t("mcph.passo_voltar")));
  } else {
    estado = t("mcph.pronto_para");
  }
  if (h && podeTs && !h.publicado && !h.funnel_ok) {
    passos.push(mcphPasso(t("mcph.passo_funnel"), h.links.funnel));
  }
  $("#mcph-dot").className = dot;
  $("#mcph-status").textContent = estado;
  $("#mcph-pronto").hidden = !ativo;
  if (ativo) {
    const tok = m.token || "";
    $("#mcph-url").textContent = ativo.url;
    $("#mcph-url-token").textContent = ativo.url + "?token=" + tok;
    $("#mcph-cabecalho").textContent = "Authorization: Bearer " + tok;
  }
  const lista = $("#mcph-passos");
  lista.replaceChildren(...passos);
  lista.hidden = !passos.length;
  // Botões: Serve/Funnel só com Tailscale pronto; ngrok fora do Android.
  const tsAtivo = h && h.publicado;
  $("#mcph-serve").hidden = !podeTs || (tsAtivo && h.publicado.modo === "serve");
  $("#mcph-funnel").hidden = !podeTs || (tsAtivo && h.publicado.modo === "funnel");
  $("#mcph-funnel").disabled = !(h && h.funnel_ok);
  $("#mcph-ngrok").hidden = !!ngrokMcp || !!(h && h.android) || !(n && n.disponivel);
  $("#mcph-desligar").hidden = !ativo;
  // Comando equivalente (para rodar à mão).
  const cmd = h && h.comandos ? (h.publicado ? h.comandos.desligar : h.comandos.serve) : "";
  $("#mcph-cmd").textContent = cmd || "";
  $("#mcph-cmd-wrap").hidden = !cmd;
  const av = $("#mcph-ngrok-aviso");
  av.hidden = !ngrokMcp;
  if (ngrokMcp) av.querySelector("span").textContent = t("ngrok.aviso_ligado").replace("{ha}", extDuracao(ngrokMcp.ha_s || 0));
}

function mcpHttpsRender(r) {
  extMcp = r;
  if (r && r.ligado) mcphCarregar();
}

async function mcphCarregar() {
  try { extHttps = await api("/api/mcp/https"); } catch (e) { extHttps = null; ieMsg("#mcp-msg", e.message, "err"); }
  mcphDesenhar();
}

async function mcphAplicar(modo, confirmado) {
  if (extOcupado) return;
  extOcupado = true;
  ieMsg("#mcp-msg", t("mcph.aplicando"), "");
  try {
    extHttps = await api("/api/mcp/https", { modo, confirmado: !!confirmado });
    ieMsg("#mcp-msg", t(modo === "off" ? "mcph.desligado" : "mcph.ligado"), "ok");
  } catch (e) { ieMsg("#mcp-msg", e.message, "err"); }
  extOcupado = false;
  mcphDesenhar();
}

// ---------- ngrok ----------
function ngrokDesenhar() {
  const n = extNgrok;
  if (!n) return;
  $("#ngrok-indisponivel").hidden = n.disponivel;
  $("#ngrok-corpo").hidden = !n.disponivel;
  $("#ngrok-link").href = n.link_authtoken;
  $("#ngrok-token-form").hidden = n.tem_authtoken;
  $("#ngrok-token-ok").hidden = !n.tem_authtoken;
  $("#ngrok-auto24").checked = !!n.auto24;
  $("#ngrok-ligar").hidden = n.ligado;
  $("#ngrok-ligar").disabled = !n.tem_authtoken;
  $("#ngrok-desligar").hidden = !n.ligado;
  document.querySelectorAll("input[name=ngrok-alvo]").forEach((i) => { i.disabled = n.ligado; if (n.ligado) i.checked = i.value === n.alvo; });
  $("#ngrok-dados").hidden = !n.ligado;
  if (n.ligado) {
    $("#ngrok-url").textContent = n.url || "";
    $("#ngrok-usuario").textContent = n.usuario || "";
    $("#ngrok-senha").textContent = n.senha || "";
    document.querySelectorAll(".ngrok-senha").forEach((e) => { e.hidden = !n.senha; });
  }
  const av = $("#ngrok-aviso");
  av.hidden = !n.ligado;
  if (n.ligado) {
    const longo = (n.ha_s || 0) >= 24 * 3600;
    av.querySelector("span").textContent = t(longo ? "ngrok.aviso_24h" : "ngrok.aviso_ligado").replace("{ha}", extDuracao(n.ha_s || 0));
  }
  if (n.erro && !n.ligado) ieMsg("#ngrok-msg", n.erro, "err");
}

async function ngrokCarregar() {
  try { extNgrok = await api("/api/ngrok"); } catch (_) { return; }
  ngrokDesenhar();
  mcphDesenhar();
}

// Pop-up obrigatório: "Entendi, ligar mesmo assim" ou "Usar o Tailscale".
function ngrokAvisar() {
  const d = $("#ngrok-dlg");
  return new Promise((ok) => {
    const fim = (v) => { d.close(); ok(v); };
    $("#ngrok-dlg-ligar").onclick = () => fim(true);
    $("#ngrok-dlg-tailscale").onclick = () => fim(false);
    d.oncancel = (e) => { e.preventDefault(); fim(false); };
    d.showModal();
    $("#ngrok-dlg-tailscale").focus();
  });
}

async function ngrokLigar(alvo, msgSel) {
  if (extOcupado) return;
  if (!await ngrokAvisar()) {
    if (alvo === "painel" && typeof irSub === "function") irSub("integracoes", "tailscale", false);
    ieMsg(msgSel, t("ngrok.use_tailscale"), "");
    return;
  }
  extOcupado = true;
  ieMsg(msgSel, t("ngrok.ligando"), "");
  try {
    extNgrok = await api("/api/ngrok", { acao: "ligar", alvo, entendi: true });
    ieMsg(msgSel, t("ngrok.ligado"), "ok");
  } catch (e) { ieMsg(msgSel, e.message, "err"); }
  extOcupado = false;
  ngrokDesenhar();
  mcphDesenhar();
}

async function ngrokDesligar(msgSel) {
  try { extNgrok = await api("/api/ngrok", { acao: "desligar" }); ieMsg(msgSel, t("ngrok.desligado"), "ok"); }
  catch (e) { ieMsg(msgSel, e.message, "err"); }
  ngrokDesenhar();
  mcphDesenhar();
}

// ---------- Início ----------
function externoAtivar(ativo) {
  clearInterval(extTimer); extTimer = null;
  if (!ativo) return;
  ngrokCarregar();
  // Só o ngrok muda sozinho (tempo ligado, queda); o Tailscale é sob demanda.
  extTimer = setInterval(ngrokCarregar, 30000);
}

function iniciarExterno() {
  document.querySelectorAll("[data-copiar]").forEach((b) => {
    const msg = b.closest("#mcp") ? "#mcp-msg" : "#ngrok-msg";
    b.addEventListener("click", () => extCopiar(b.dataset.copiar, msg));
  });
  $("#mcph-serve").onclick = () => mcphAplicar("serve");
  $("#mcph-funnel").onclick = async () => {
    if (!await confirmar(t("mcph.confirma_funnel"), t("mcph.confirma_funnel_sim"))) return;
    mcphAplicar("funnel", true);
  };
  $("#mcph-desligar").onclick = () => {
    if (extHttps && extHttps.publicado) mcphAplicar("off");
    else ngrokDesligar("#mcp-msg");
  };
  $("#mcph-ngrok").onclick = () => {
    if (!extNgrok || !extNgrok.tem_authtoken) {
      if (typeof irSub === "function") irSub("integracoes", "tailscale", false);
      ieMsg("#ngrok-msg", t("ngrok.falta_token"), "err");
      return;
    }
    ngrokLigar("mcp", "#mcp-msg");
  };
  $("#ngrok-token-form").onsubmit = async (e) => {
    e.preventDefault();
    const v = e.target.authtoken.value.trim();
    if (!v) return;
    try { extNgrok = await api("/api/ngrok", { authtoken: v }); e.target.reset(); ieMsg("#ngrok-msg", t("ngrok.token_guardado"), "ok"); }
    catch (err) { ieMsg("#ngrok-msg", err.message, "err"); }
    ngrokDesenhar();
  };
  $("#ngrok-token-trocar").onclick = () => { $("#ngrok-token-form").hidden = false; $("#ngrok-token-ok").hidden = true; $("#ngrok-token-form").authtoken.focus(); };
  $("#ngrok-auto24").onchange = async (e) => {
    try { extNgrok = await api("/api/ngrok", { auto24: e.target.checked }); } catch (err) { e.target.checked = !e.target.checked; ieMsg("#ngrok-msg", err.message, "err"); }
  };
  $("#ngrok-ligar").onclick = () => {
    const alvo = (document.querySelector("input[name=ngrok-alvo]:checked") || {}).value || "painel";
    ngrokLigar(alvo, "#ngrok-msg");
  };
  $("#ngrok-desligar").onclick = () => ngrokDesligar("#ngrok-msg");
}

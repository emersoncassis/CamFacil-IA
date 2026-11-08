// CamFácil IA — fase 6 na aba Integrações: botões de portão/relé (cadastro,
// PIN e log) e MCP (desligado por padrão). Usa api, el, t, $ e tp/ieMsg dos
// outros scripts. JS puro.
"use strict";

let f6Timer = null;

function f6Data(unix) {
  return new Date(unix * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

// ---------- Botões de portão e relé ----------
async function renderBotoes() {
  let r;
  try { r = await api("/api/botoes"); } catch (e) { ieMsg("#bt-msg", e.message, "err"); return; }
  $("#bt-pin-situacao").textContent = r.travado_s
    ? t("bt.travado").replace("{s}", r.travado_s)
    : t(r.tem_pin ? "bt.pin_definido" : "bt.pin_falta");
  const atual = $("#bt-pin-form").atual;
  atual.hidden = !r.tem_pin; atual.required = r.tem_pin;
  const corpo = $("#bt-lista");
  if (!r.botoes.length) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "3" }, t("bt.nenhum"))));
  } else {
    corpo.replaceChildren(...r.botoes.map((b) => {
      const rem = el("button", { class: "btn ghost sm", type: "button" }, t("bt.remover"));
      rem.onclick = async () => {
        if (!await confirmar(t("bt.confirma_remover").replace("{nome}", b.nome))) return;
        try { await api(`/api/botoes/${b.id}`, undefined, "DELETE"); renderBotoes(); } catch (e) { ieMsg("#bt-msg", e.message, "err"); }
      };
      return el("tr", {},
        el("td", {}, b.nome, el("span", { class: "tiny" }, " · " + t("bt.icone." + b.icone))),
        el("td", { class: "mono" }, b.endereco ? `${b.metodo} ${b.endereco}${b.com_senha ? " · " + t("bt.com_senha") : ""}` : t("bt.ilegivel")),
        el("td", { class: "right" }, rem));
    }));
  }
  renderBotoesLog();
}

async function renderBotoesLog() {
  let r;
  try { r = await api("/api/botoes/log"); } catch (_) { return; }
  const lista = $("#bt-log");
  if (!r.log.length) { lista.replaceChildren(el("li", {}, el("p", {}, t("bt.log_vazio")))); return; }
  lista.replaceChildren(...r.log.slice(0, 15).map((e) => el("li", {},
    el("div", {}, el("strong", {}, t(e.tipo === "acionamento" ? "bt.log_acionado" : "bt.log_negado")), el("p", {}, e.detalhe)),
    el("span", { class: "tiny mono" }, f6Data(e.quando)))));
}

function iniciarBotoes() {
  $("#bt-form").onsubmit = async (ev) => {
    ev.preventDefault();
    const f = ev.target;
    ieMsg("#bt-msg", "");
    try {
      await api("/api/botoes", {
        nome: f.nome.value.trim(), icone: f.icone.value, url: f.url.value.trim(), metodo: f.metodo.value,
        usuario: f.usuario.value.trim(), senha: f.senha.value, corpo: f.corpo.value,
      });
      f.reset();
      ieMsg("#bt-msg", t("bt.salvo"), "ok");
      renderBotoes();
    } catch (e) { ieMsg("#bt-msg", e.message, "err"); }
  };
  $("#bt-pin-form").onsubmit = async (ev) => {
    ev.preventDefault();
    const f = ev.target;
    try {
      await api("/api/botoes/pin", { atual: f.atual.value, novo: f.novo.value });
      f.reset();
      ieMsg("#bt-msg", t("bt.pin_ok"), "ok");
      renderBotoes();
    } catch (e) { ieMsg("#bt-msg", e.message, "err"); }
  };
}

// ---------- MCP ----------
async function renderMcp(r) {
  if (!r) { try { r = await api("/api/mcp"); } catch (e) { ieMsg("#mcp-msg", e.message, "err"); return; } }
  $("#mcp-ligado").checked = r.ligado;
  $("#mcp-escrita").checked = r.escrita;
  $("#mcp-escrita").disabled = !r.ligado;
  $("#mcp-detalhes").hidden = !r.ligado;
  $("#mcp-url").textContent = r.url;
  $("#mcp-token").textContent = r.token || "—";
  $("#mcp-exemplo").textContent = r.exemplo;
  $("#mcp-claude-code").textContent = r.claude_code || "";
  if (typeof mcpHttpsRender === "function") mcpHttpsRender(r);
}

function iniciarMcp() {
  $("#mcp-ligado").onchange = async (e) => {
    const ligar = e.target.checked;
    if (ligar && !await confirmar(t("mcp.confirma_ligar"))) { e.target.checked = false; return; }
    try { renderMcp(await api("/api/mcp", { ligado: ligar })); ieMsg("#mcp-msg", t(ligar ? "mcp.ligado" : "mcp.desligado"), "ok"); }
    catch (err) { e.target.checked = !ligar; ieMsg("#mcp-msg", err.message, "err"); }
  };
  $("#mcp-escrita").onchange = async (e) => {
    const sim = e.target.checked;
    if (sim && !await confirmar(t("mcp.confirma_escrita"))) { e.target.checked = false; return; }
    try { renderMcp(await api("/api/mcp", { escrita: sim })); }
    catch (err) { e.target.checked = !sim; ieMsg("#mcp-msg", err.message, "err"); }
  };
  $("#mcp-trocar").onclick = async () => {
    if (!await confirmar(t("mcp.confirma_trocar"))) return;
    try { renderMcp(await api("/api/mcp/token", {})); ieMsg("#mcp-msg", t("mcp.token_trocado"), "ok"); }
    catch (e) { ieMsg("#mcp-msg", e.message, "err"); }
  };
  $("#mcp-copiar").onclick = async () => {
    try { await navigator.clipboard.writeText($("#mcp-token").textContent); ieMsg("#mcp-msg", t("mcp.copiado"), "ok"); }
    catch (_) { /* sem permissão de área de transferência */ }
  };
}

function fase6Ativar(ativo) {
  clearInterval(f6Timer); f6Timer = null;
  if (ativo) { renderBotoes(); renderMcp(); f6Timer = setInterval(renderBotoesLog, 10000); }
  if (typeof externoAtivar === "function") externoAtivar(ativo);
}

function iniciarFase6() {
  iniciarBotoes();
  iniciarMcp();
  if (typeof iniciarExterno === "function") iniciarExterno();
  const painel = $("#integracoes");
  new MutationObserver(() => fase6Ativar(!painel.hidden)).observe(painel, { attributes: true, attributeFilter: ["hidden"] });
  fase6Ativar(!painel.hidden);
}

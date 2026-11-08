// CamFácil IA — integrações da fase 3b: WhatsApp (Evolution API), webhook,
// termos/LGPD e arquivo de alertas. Usa `api`, `el`, `t`, `$`, `busy` e
// `tempoRelativo` do app.js. JS puro, sem framework.
"use strict";

function ieMsg(sel, texto, tipo) { const m = $(sel); if (!m) return; m.textContent = texto; m.className = "msg " + (tipo || ""); }

// ---------- WhatsApp ----------
let waTimer = null;

async function renderWhatsApp() {
  let r;
  try { r = await api("/api/whatsapp"); } catch (e) { ieMsg("#wa-msg", e.message, "err"); return; }
  const s = r.situacao;
  const dot = $("#wa-dot");
  let texto;
  if (!s.configurado) { dot.className = "dot"; texto = t("wa.sem_config"); }
  else if (s.conexao === "Conectado") { dot.className = "dot on"; texto = t("wa.conectado"); }
  else if (s.conexao === "Inexistente") { dot.className = "dot off"; texto = t("wa.inexistente"); }
  else { dot.className = "dot off"; texto = s.ultimo_erro || t("wa.desconectado"); }
  $("#wa-status").textContent = texto;
  $("#wa-inst").textContent = s.instancia ? `${s.instancia} · ${s.url}` : "—";
  $("#wa-envios").textContent = s.configurado
    ? `${s.enviados} ${t("tg.enviados")} · ${s.na_fila} ${t("tg.fila")} · ${s.recebidas} ${t("wa.recebidas")}${s.descartados ? ` · ${s.descartados} ${t("tg.descartados")}` : ""}` : "—";
  $("#wa-form").hidden = s.configurado;
  $("#wa-acoes").hidden = !s.configurado;
  $("#wa-receber").hidden = !s.configurado;
  $("#wa-contatos").hidden = !s.configurado;
  $("#wa-fotos").checked = s.fotos;
  $("#wa-fotos-wrap").title = t("tg.fotos_dica");
  if (r.webhook_url) $("#wa-webhook-url").textContent = r.webhook_url;
  const corpo = $("#wa-lista");
  if (!r.contatos.length) { corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "5" }, t("wa.nenhum_contato")))); return; }
  const agora = Math.floor(Date.now() / 1000);
  corpo.replaceChildren(...r.contatos.map((c) => {
    const numero = c.jid.replace(/@s\.whatsapp\.net$/, "");
    const chk = el("input", { type: "checkbox", "aria-label": `${t("tg.col_autorizado")}: ${c.nome || numero}` });
    chk.checked = c.autorizado;
    chk.onchange = async () => {
      try { await api(`/api/whatsapp/contatos/${encodeURIComponent(c.jid)}`, { autorizado: chk.checked }); renderWhatsApp(); }
      catch (e) { chk.checked = !chk.checked; ieMsg("#wa-msg", e.message, "err"); }
    };
    const rem = el("button", { class: "btn ghost sm", type: "button" }, t("wa.remover_contato"));
    rem.onclick = async () => {
      if (!await confirmar(t("wa.confirma_remover_contato"))) return;
      await api(`/api/whatsapp/contatos/${encodeURIComponent(c.jid)}`, undefined, "DELETE"); renderWhatsApp();
    };
    return el("tr", {},
      el("td", {}, c.nome || "—"),
      el("td", { class: "mono" }, numero),
      el("td", {}, `${tempoRelativo(agora - c.visto_em)} ${t("tempo.atras")}`),
      el("td", {}, el("label", { class: "switch" }, chk, el("span"))),
      el("td", { class: "right" }, rem));
  }));
}

async function waSalvar(ev) {
  ev.preventDefault();
  const f = ev.target; const b = $("#wa-salvar");
  const dados = { url: f.url.value.trim(), instancia: f.instancia.value.trim(), apikey: f.apikey.value.trim() };
  if (!dados.url || !dados.instancia || !dados.apikey) return;
  busy(b, true, "wa.salvando"); ieMsg("#wa-msg", "");
  try {
    await api("/api/whatsapp", dados);
    f.apikey.value = "";
    ieMsg("#wa-msg", t("wa.salvo"), "ok");
    renderWhatsApp();
  } catch (e) { ieMsg("#wa-msg", e.message, "err"); }
  busy(b, false, "wa.salvar");
}

async function waQr() {
  const b = $("#wa-qr-btn");
  busy(b, true, "wa.gerando"); ieMsg("#wa-msg", "");
  try {
    const r = await api("/api/whatsapp/qr", {});
    if (r.conectado) { $("#wa-qr-area").hidden = true; ieMsg("#wa-msg", t("wa.ja_conectado"), "ok"); }
    else { $("#wa-qr").src = r.qr; $("#wa-qr-area").hidden = false; }
    renderWhatsApp();
  } catch (e) { ieMsg("#wa-msg", e.message, "err"); }
  busy(b, false, "wa.gerar_qr");
}

async function waTestar() {
  const b = $("#wa-testar");
  busy(b, true, "wa.testando"); ieMsg("#wa-msg", "");
  try {
    const r = await api("/api/whatsapp/testar", {});
    if (r.conexao !== "Conectado") ieMsg("#wa-msg", t("wa.sem_conexao"), "err");
    else ieMsg("#wa-msg", `${t("wa.testado")} ${r.enviados} ${r.enviados === 1 ? t("tg.contato") : t("tg.contatos_n")}.`, "ok");
  } catch (e) { ieMsg("#wa-msg", e.message, "err"); }
  busy(b, false, "wa.testar");
  renderWhatsApp();
}

async function waRegistrar(ev) {
  ev.preventDefault();
  const b = $("#wa-wh-registrar");
  busy(b, true, "wa.registrando"); ieMsg("#wa-wh-msg", "");
  try {
    const r = await api("/api/whatsapp/webhook/registrar", { url_base: ev.target.url_base.value.trim() || null });
    ieMsg("#wa-wh-msg", `${t("wa.registrado")} ${r.url}`, "ok");
  } catch (e) { ieMsg("#wa-wh-msg", e.message, "err"); }
  busy(b, false, "wa.registrar");
}

function iniciarWhatsApp() {
  $("#wa-form").onsubmit = waSalvar;
  $("#wa-qr-btn").onclick = waQr;
  $("#wa-testar").onclick = waTestar;
  $("#wa-wh-form").onsubmit = waRegistrar;
  $("#wa-remover").onclick = async () => {
    if (!await confirmar(t("wa.confirma_remover"))) return;
    try { await api("/api/whatsapp", undefined, "DELETE"); $("#wa-qr-area").hidden = true; ieMsg("#wa-msg", t("wa.removido"), "ok"); renderWhatsApp(); }
    catch (e) { ieMsg("#wa-msg", e.message, "err"); }
  };
  $("#wa-fotos").onchange = async (e) => { try { await api("/api/whatsapp/fotos", { ligado: e.target.checked }); } catch (err) { ieMsg("#wa-msg", err.message, "err"); } };
  $("#wa-copiar").onclick = async () => { try { await navigator.clipboard.writeText($("#wa-webhook-url").textContent); ieMsg("#wa-wh-msg", t("wa.copiado"), "ok"); } catch (_) { /* sem permissão */ } };
}

// ---------- Webhook genérico ----------
let whEditando = false;

async function renderWebhook() {
  let r;
  try { r = await api("/api/webhook"); } catch (e) { ieMsg("#wh-msg", e.message, "err"); return; }
  const s = r.situacao;
  const dot = $("#wh-dot");
  if (!s.configurado) { dot.className = "dot"; $("#wh-status").textContent = t("wh.sem_config"); }
  else if (s.ultimo_erro) { dot.className = "dot off"; $("#wh-status").textContent = s.ultimo_erro; }
  else { dot.className = "dot on"; $("#wh-status").textContent = `${t("wh.ativo")} · ${s.url}${s.cabecalho_nome ? ` · ${t("wh.cabecalho")} ${s.cabecalho_nome}` : ""}`; }
  $("#wh-envios").textContent = s.configurado ? `${s.enviados} ${t("wh.enviados")} · ${s.falhas} ${t("wh.falhas")}` : "—";
  $("#wh-form").hidden = s.configurado && !whEditando;
  $("#wh-acoes").hidden = !s.configurado;
  if (whEditando && s.url && !$("#wh-form").url.value) $("#wh-form").url.value = s.url;
  $("#wh-exemplo").textContent = JSON.stringify(r.exemplo, null, 2);
}

async function whSalvar(ev) {
  ev.preventDefault();
  const f = ev.target;
  ieMsg("#wh-msg", "");
  try {
    await api("/api/webhook", { url: f.url.value.trim(), cabecalho: f.cabecalho.value.trim() });
    f.cabecalho.value = ""; whEditando = false;
    ieMsg("#wh-msg", t("wh.salvo"), "ok");
    renderWebhook();
  } catch (e) { ieMsg("#wh-msg", e.message, "err"); }
}

async function whTestar() {
  const b = $("#wh-testar");
  busy(b, true, "wh.testando"); ieMsg("#wh-msg", "");
  try { const r = await api("/api/webhook/testar", {}); ieMsg("#wh-msg", `${t("wh.testado")} ${r.status}.`, "ok"); }
  catch (e) { ieMsg("#wh-msg", e.message, "err"); }
  busy(b, false, "wh.testar");
  renderWebhook();
}

function iniciarWebhook() {
  $("#wh-form").onsubmit = whSalvar;
  $("#wh-testar").onclick = whTestar;
  $("#wh-editar").onclick = () => { whEditando = true; renderWebhook(); $("#wh-form").url.focus(); };
  $("#wh-remover").onclick = async () => {
    if (!await confirmar(t("wh.confirma_remover"))) return;
    try { await api("/api/webhook", undefined, "DELETE"); ieMsg("#wh-msg", t("wh.removido"), "ok"); renderWebhook(); }
    catch (e) { ieMsg("#wh-msg", e.message, "err"); }
  };
}

// ---------- Termos e LGPD ----------
function ieData(unix) { return unix ? new Date(unix * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) : "—"; }

async function renderTermos() {
  let r;
  try { r = await api("/api/termos"); } catch (e) { ieMsg("#lg-msg", e.message, "err"); return; }
  const tec = r.tecnico;
  const dot = $("#lg-dot");
  if (tec.aceito) { dot.className = "dot on"; $("#lg-status").textContent = `${t("lg.aceito_em").replace("{v}", tec.versao_aceita)} ${ieData(tec.quando)}`; }
  else if (tec.versao_aceita) { dot.className = "dot off"; $("#lg-status").textContent = t("lg.versao_nova"); }
  else { dot.className = "dot off"; $("#lg-status").textContent = t("lg.nao_aceito"); }
  $("#lg-aceitar").hidden = tec.aceito;
  $("#lg-texto").textContent = tec.texto;
  const cli = r.cliente;
  $("#lg-cliente-status").textContent = cli.registrado
    ? `${t("lg.cliente_registrado")} ${ieData(cli.quando)}${cli.dados && cli.dados.cliente ? ` · ${cli.dados.cliente}` : ""}`
    : t("lg.cliente_pendente");
  const f = $("#lg-form");
  if (cli.dados && !f.dataset.preenchido) {
    for (const k of ["cliente", "documento", "tecnico", "contato", "local"]) f[k].value = cli.dados[k] || "";
    f.prazo_dias.value = cli.dados.prazo_dias || "";
    f.dataset.preenchido = "1";
  }
}

function iniciarTermos() {
  $("#lg-aceitar").onclick = async () => {
    ieMsg("#lg-msg", "");
    try { await api("/api/termos/tecnico", {}); ieMsg("#lg-msg", t("lg.aceito_ok"), "ok"); renderTermos(); }
    catch (e) { ieMsg("#lg-msg", e.message, "err"); }
  };
  $("#lg-form").onsubmit = async (ev) => {
    ev.preventDefault();
    const f = ev.target;
    ieMsg("#lg-msg", "");
    try {
      await api("/api/termos/cliente", {
        cliente: f.cliente.value, documento: f.documento.value, tecnico: f.tecnico.value,
        contato: f.contato.value, local: f.local.value, prazo_dias: parseInt(f.prazo_dias.value, 10) || 0,
      });
      ieMsg("#lg-msg", t("lg.cliente_ok"), "ok");
      renderTermos();
    } catch (e) { ieMsg("#lg-msg", e.message, "err"); }
  };
  $("#lg-apagar").onclick = async () => {
    const c = prompt(t("lg.apagar_confirma"));
    if (c == null) return;
    ieMsg("#lg-msg", "");
    try {
      const r = await api("/api/termos/apagar", { confirmacao: c });
      ieMsg("#lg-msg", t("lg.apagado").replace("{e}", r.eventos).replace("{a}", r.arquivados).replace("{f}", r.fotos), "ok");
      renderArquivo();
    } catch (e) { ieMsg("#lg-msg", e.message, "err"); }
  };
}

// ---------- Arquivo de alertas ----------
async function renderArquivo(comForm) {
  let r;
  try { r = await api("/api/arquivo"); } catch (e) { ieMsg("#arq-msg", e.message, "err"); return; }
  const dot = $("#arq-dot");
  if (!r.ligado) { dot.className = "dot"; $("#arq-status").textContent = t("arq.desligado"); }
  else if (r.ultimo_erro) { dot.className = "dot off"; $("#arq-status").textContent = r.ultimo_erro; }
  else { dot.className = "dot on"; $("#arq-status").textContent = t(r.chat_id ? "arq.ligado_canal" : "arq.ligado_local"); }
  if (comForm) {
    const f = $("#arq-form");
    f.ligado.checked = r.ligado;
    f.manter.value = r.manter;
    f.chat_id.value = r.chat_id || "";
  }
  const corpo = $("#arq-lista");
  if (!r.alertas.length) { corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "4" }, t("arq.vazio")))); return; }
  corpo.replaceChildren(...r.alertas.map((a) => {
    const links = el("td", { class: "right" });
    if (a.foto) links.append(el("a", { class: "btn ghost sm", href: a.foto, target: "_blank", rel: "noopener" }, t("arq.ver_foto")));
    if (a.telegram) links.append(el("a", { class: "btn ghost sm", href: a.telegram, target: "_blank", rel: "noopener" }, t("arq.ver_telegram")));
    return el("tr", {}, el("td", { class: "mono" }, a.data), el("td", {}, a.camera), el("td", {}, a.regra || t("arq.saude")), links);
  }));
}

function iniciarArquivo() {
  $("#arq-form").onsubmit = async (ev) => {
    ev.preventDefault();
    const f = ev.target;
    ieMsg("#arq-msg", "");
    try {
      await api("/api/arquivo", { ligado: f.ligado.checked, chat_id: f.chat_id.value.trim(), manter: parseInt(f.manter.value, 10) || null });
      ieMsg("#arq-msg", t("arq.salvo"), "ok");
      renderArquivo(true);
    } catch (e) { ieMsg("#arq-msg", e.message, "err"); renderArquivo(true); }
  };
}

// ---------- Ativação junto com a aba Integrações ----------
// showTab (app.js) só esconde/mostra o painel; aqui observamos o atributo
// `hidden` para ligar e desligar a atualização periódica sem mexer no app.js.
function integracoesExtraAtivar(ativo) {
  clearInterval(waTimer); waTimer = null;
  if (ativo) {
    renderTermos(); renderArquivo(true);
    renderWhatsApp(); renderWebhook();
    waTimer = setInterval(() => { renderWhatsApp(); renderWebhook(); renderArquivo(false); }, 5000);
  }
}

function iniciarIntegracoesExtra() {
  iniciarWhatsApp();
  iniciarWebhook();
  iniciarTermos();
  iniciarArquivo();
  const painel = $("#integracoes");
  new MutationObserver(() => integracoesExtraAtivar(!painel.hidden)).observe(painel, { attributes: true, attributeFilter: ["hidden"] });
  integracoesExtraAtivar(!painel.hidden);
}

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
      if (!confirm(t("wa.confirma_remover_contato"))) return;
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
    if (!confirm(t("wa.confirma_remover"))) return;
    try { await api("/api/whatsapp", undefined, "DELETE"); $("#wa-qr-area").hidden = true; ieMsg("#wa-msg", t("wa.removido"), "ok"); renderWhatsApp(); }
    catch (e) { ieMsg("#wa-msg", e.message, "err"); }
  };
  $("#wa-fotos").onchange = async (e) => { try { await api("/api/whatsapp/fotos", { ligado: e.target.checked }); } catch (err) { ieMsg("#wa-msg", err.message, "err"); } };
  $("#wa-copiar").onclick = async () => { try { await navigator.clipboard.writeText($("#wa-webhook-url").textContent); ieMsg("#wa-wh-msg", t("wa.copiado"), "ok"); } catch (_) { /* sem permissão */ } };
}

// ---------- Ativação junto com a aba Integrações ----------
// showTab (app.js) só esconde/mostra o painel; aqui observamos o atributo
// `hidden` para ligar e desligar a atualização periódica sem mexer no app.js.
function integracoesExtraAtivar(ativo) {
  clearInterval(waTimer); waTimer = null;
  if (ativo) { renderWhatsApp(); waTimer = setInterval(renderWhatsApp, 5000); }
}

function iniciarIntegracoesExtra() {
  iniciarWhatsApp();
  const painel = $("#integracoes");
  new MutationObserver(() => integracoesExtraAtivar(!painel.hidden)).observe(painel, { attributes: true, attributeFilter: ["hidden"] });
  integracoesExtraAtivar(!painel.hidden);
}

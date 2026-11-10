// CamFácil IA — entrada para quem acessa pela rede (ADR 0017).
// Dois caminhos: PIN (mostrado só na tela da máquina) ou pedido com código
// conferido dos dois lados (a máquina clica em "Autorizar"). JS puro.
"use strict";

(async () => {
  let T = {};
  const t = (k) => T[k] ?? k;
  const $ = (s) => document.querySelector(s);
  try { T = await (await fetch("/i18n/pt-BR.json")).json(); } catch (_) { /* segue com as chaves */ }
  document.querySelectorAll("[data-i18n]").forEach((e) => { e.textContent = t(e.dataset.i18n); });
  document.querySelectorAll("[data-i18n-ph]").forEach((e) => { e.placeholder = t(e.dataset.i18nPh); });

  const msg = (texto, tipo) => { const m = $("#msg"); m.textContent = texto || ""; m.className = "msg " + (tipo || ""); };

  // Para onde voltar depois de entrar: só caminhos desta dashboard.
  const volta = (() => {
    const v = new URLSearchParams(location.search).get("volta") || "/";
    return /^\/(?!\/)[A-Za-z0-9/_.-]*$/.test(v) && !v.startsWith("/entrar") ? v : "/";
  })();
  const seguir = () => { location.replace(volta); };

  try {
    const eu = await (await fetch("/api/acesso/eu")).json();
    if (eu.liberado) { seguir(); return; }
    if (eu.bloqueado_geral) msg(t("ent.bloqueado_geral"), "err");
  } catch (_) { /* mostra a tela */ }

  // Nome do aparelho: lembrado neste navegador (só conveniência).
  const nome = $("#nome");
  try { nome.value = localStorage.getItem("cf_nome_aparelho") || ""; } catch (_) { /* sem storage */ }
  if (!nome.value) nome.value = /Android|iPhone|iPad/i.test(navigator.userAgent) ? t("ent.nome_celular") : t("ent.nome_computador");
  const guardarNome = () => { try { localStorage.setItem("cf_nome_aparelho", nome.value.trim()); } catch (_) { /* sem storage */ } };

  async function post(caminho, corpo) {
    const r = await fetch(caminho, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(corpo) });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.erro || r.statusText);
    return d;
  }

  // ---------- Abas ----------
  function aba(qual) {
    $("#aba-pin").setAttribute("aria-selected", String(qual === "pin"));
    $("#aba-pedir").setAttribute("aria-selected", String(qual === "pedir"));
    $("#f-pin").hidden = qual !== "pin";
    $("#f-pedir").hidden = qual !== "pedir";
    msg("");
  }
  $("#aba-pin").onclick = () => aba("pin");
  $("#aba-pedir").onclick = () => aba("pedir");

  // ---------- PIN ----------
  $("#f-pin").onsubmit = async (e) => {
    e.preventDefault();
    const b = e.submitter || $("#f-pin button");
    b.disabled = true;
    msg(t("ent.conferindo"));
    try {
      guardarNome();
      await post("/api/acesso/entrar", { pin: $("#pin").value.trim(), nome: nome.value.trim() });
      msg(t("ent.liberado"), "ok");
      seguir();
    } catch (err) {
      msg(err.message, "err");
      $("#pin").value = "";
      $("#pin").focus();
    } finally {
      b.disabled = false;
    }
  };

  // ---------- Pedido com confirmação dos dois lados ----------
  let timer = null;
  function parar() { if (timer) clearTimeout(timer); timer = null; }
  function voltarAoInicio() { parar(); $("#pedir-espera").hidden = true; $("#pedir-inicio").hidden = false; }

  $("#pedir").onclick = async () => {
    msg("");
    try {
      guardarNome();
      const p = await post("/api/acesso/pedir", { nome: nome.value.trim() });
      $("#codigo").textContent = p.codigo.replace(/(\d{3})(\d{3})/, "$1 $2");
      $("#pedir-inicio").hidden = true;
      $("#pedir-espera").hidden = false;
      const ver = async () => {
        let r = {};
        try { r = await (await fetch(`/api/acesso/pedido/${encodeURIComponent(p.id)}?s=${encodeURIComponent(p.segredo)}`)).json(); } catch (_) { /* tenta de novo */ }
        if (r.estado === "aprovado") { msg(t("ent.liberado"), "ok"); seguir(); return; }
        if (r.estado === "recusado") { voltarAoInicio(); msg(t("ent.recusado"), "err"); return; }
        if (r.estado === "expirado") { voltarAoInicio(); msg(t("ent.expirou"), "err"); return; }
        if (r.expira_s != null) $("#espera").textContent = t("ent.esperando").replace("{s}", r.expira_s);
        timer = setTimeout(ver, 2000);
      };
      ver();
    } catch (err) {
      msg(err.message, "err");
    }
  };
  $("#cancelar").onclick = () => { voltarAoInicio(); msg(""); };
})();

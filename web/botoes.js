// CamFácil IA — botões de portão e relé (celular / app Android).
// Fluxo: toca no botão → PIN → "Confirmar?" → aciona. O servidor confere o
// PIN, dá um código de uso único (20 s) e grava o log. JS puro.
"use strict";

(async () => {
  let T = {};
  const t = (k) => T[k] ?? k;
  const $ = (s) => document.querySelector(s);
  try { T = await (await fetch("/i18n/pt-BR.json")).json(); } catch (_) { /* segue com as chaves */ }
  document.querySelectorAll("[data-i18n]").forEach((e) => { e.textContent = t(e.dataset.i18n); });
  document.querySelectorAll("[data-i18n-ph]").forEach((e) => { e.placeholder = t(e.dataset.i18nPh); });

  const ICONES = {
    portao: "M3 21V9l9-6 9 6v12M7 21v-8h10v8M7 17h10",
    rele: "M7 3h10v18H7zM12 7v4M10 15h4",
    luz: "M9 18h6M10 21h4M12 3a6 6 0 0 0-4 10.5c.7.7 1 1.5 1 2.5h6c0-1 .3-1.8 1-2.5A6 6 0 0 0 12 3z",
  };
  const ns = "http://www.w3.org/2000/svg";
  const msg = (texto, tipo) => { const m = $("#msg"); m.textContent = texto || ""; m.className = "msg " + (tipo || ""); };
  const dmsg = (texto) => { $("#dlg-msg").textContent = texto || ""; $("#dlg-msg").className = "msg err"; };

  async function post(caminho, corpo) {
    const r = await fetch(caminho, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(corpo) });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.erro || r.statusText);
    return d;
  }

  const quem = $("#quem");
  try { quem.value = localStorage.getItem("cf-quem") || ""; } catch (_) { /* opcional */ }
  quem.onchange = () => { try { localStorage.setItem("cf-quem", quem.value.trim()); } catch (_) { /* opcional */ } };

  const dlg = $("#dlg");
  let atual = null;
  let confirmacao = null;
  let relogio = null;

  function fechar() {
    clearInterval(relogio); relogio = null;
    confirmacao = null; atual = null;
    $("#pin").value = ""; dmsg("");
    if (dlg.open) dlg.close();
  }

  function abrir(b) {
    atual = b;
    $("#dlg-titulo").textContent = b.nome;
    $("#dlg-pin").hidden = false;
    $("#dlg-confirmar").hidden = true;
    dmsg("");
    dlg.showModal();
    $("#pin").focus();
  }

  $("#dlg-cancelar").onclick = fechar;
  $("#dlg-nao").onclick = fechar;
  dlg.addEventListener("close", () => { clearInterval(relogio); relogio = null; });

  $("#dlg-pin").onsubmit = async (ev) => {
    ev.preventDefault();
    try {
      const r = await post(`/api/botoes/${atual.id}/preparar`, { pin: $("#pin").value, quem: quem.value.trim() });
      confirmacao = r.confirmacao;
      $("#pin").value = "";
      $("#dlg-pin").hidden = true;
      $("#dlg-confirmar").hidden = false;
      $("#dlg-pergunta").textContent = t("bt.pergunta").replace("{nome}", r.nome);
      let resta = r.expira_s;
      const tick = () => {
        $("#dlg-expira").textContent = t("bt.expira").replace("{s}", resta);
        if (resta-- <= 0) { fechar(); msg(t("bt.expirou"), "err"); }
      };
      tick();
      relogio = setInterval(tick, 1000);
      $("#dlg-sim").focus();
    } catch (e) { dmsg(e.message); $("#pin").select(); }
  };

  $("#dlg-sim").onclick = async () => {
    const b = atual; const c = confirmacao;
    fechar();
    if (!b || !c) return;
    msg(t("bt.acionando"));
    try {
      await post(`/api/botoes/${b.id}/acionar`, { confirmacao: c });
      msg(t("bt.acionado").replace("{nome}", b.nome), "ok");
    } catch (e) { msg(e.message, "err"); }
  };

  let lista = [];
  try {
    const r = await (await fetch("/api/botoes")).json();
    lista = r.botoes || [];
    if (r.travado_s) msg(t("bt.travado").replace("{s}", r.travado_s), "err");
    else if (!r.tem_pin && lista.length) msg(t("bt.pin_falta"), "err");
  } catch (_) { msg(t("bt.sem_conexao"), "err"); }
  const grade = $("#grade");
  if (!lista.length) {
    const p = document.createElement("p");
    p.className = "vazio";
    p.textContent = t("bt.nenhum");
    grade.append(p);
    return;
  }
  const botoes = lista.map((b) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "botao";
    const svg = document.createElementNS(ns, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    const p = document.createElementNS(ns, "path");
    p.setAttribute("d", ICONES[b.icone] || ICONES.portao);
    svg.append(p);
    const s = document.createElement("span");
    s.textContent = b.nome;
    btn.append(svg, s);
    btn.onclick = () => abrir(b);
    return btn;
  });
  grade.append(...botoes);

  // Zero rolagem: quantos botões cabem na grade; o resto vai para a próxima página.
  let pag = 0;
  $("#ant").setAttribute("aria-label", t("pag.anterior"));
  $("#prox").setAttribute("aria-label", t("pag.proxima"));
  function paginar(segunda) {
    botoes.forEach((b) => { b.hidden = false; });
    const cs = getComputedStyle(grade);
    const cols = Math.max(1, cs.gridTemplateColumns.split(" ").length);
    const gap = parseFloat(cs.rowGap) || 0;
    const linhas = Math.max(1, Math.floor((grade.clientHeight + gap) / (120 + gap)));
    const n = cols * linhas;
    const paginas = Math.ceil(botoes.length / n);
    pag = Math.max(0, Math.min(pag, paginas - 1));
    botoes.forEach((b, i) => { b.hidden = Math.floor(i / n) !== pag; });
    $("#pager").hidden = paginas < 2;
    $("#pag-txt").textContent = `${pag + 1} ${t("pag.de")} ${paginas}`;
    $("#ant").disabled = pag === 0;
    $("#prox").disabled = pag >= paginas - 1;
    // A barra de páginas ocupa espaço: mede de novo com ela no lugar.
    if (!segunda && paginas > 1) paginar(true);
  }
  $("#ant").onclick = () => { pag--; paginar(); };
  $("#prox").onclick = () => { pag++; paginar(); };
  addEventListener("resize", () => paginar());
  paginar();
})();

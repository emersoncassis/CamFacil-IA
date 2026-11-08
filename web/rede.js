// CamFácil IA — tela Rede (SDD-05). Usa t(), $, el(), api() e tempoRelativo() de app.js.
"use strict";

let redeTimer = null;
let redeUltima = null;

function redeData(unix) {
  return new Date(unix * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

function redeTipo(d) {
  const chave = d.eu ? "computador" : d.tipo;
  const s = el("span", { class: "rede-tipo " + chave }, t("rede.tipo." + chave));
  if (d.eu) s.append(el("span", { class: "rede-marca" }, t("rede.eu")));
  if (d.gateway) s.append(el("span", { class: "rede-marca" }, t("rede.gateway")));
  if (d.onvif) s.append(el("span", { class: "rede-marca" }, "ONVIF"));
  return s;
}

function redeRenderInventario(foto) {
  const corpo = $("#rede-inventario");
  if (!foto) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "6" }, t("rede.sem_foto"))));
    $("#rede-meta").textContent = "";
    return;
  }
  $("#rede-meta").textContent = `${foto.rede} · ${redeData(foto.quando)} · ${foto.dispositivos.length} ${t("rede.aparelhos")}`;
  if (!foto.dispositivos.length) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "6" }, t("rede.vazia"))));
    return;
  }
  corpo.replaceChildren(...foto.dispositivos.map((d) => el("tr", { class: d.eu ? "sou-eu" : "" },
    el("td", { class: "mono" }, d.ip),
    el("td", {}, redeTipo(d)),
    el("td", {}, d.fabricante || "—", d.modelo ? el("span", { class: "tiny" }, " · " + d.modelo) : null),
    el("td", { class: "mono" }, d.mac || "—"),
    el("td", { class: "mono" }, d.portas.length ? d.portas.join(" ") : "—"),
    el("td", {}, d.camera_nome ? d.camera_nome : el("span", { class: "tiny" }, "—")))));
}

function redeRenderMudancas(r) {
  const desc = $("#rede-mudancas-desc");
  const lista = $("#rede-mudancas");
  if (!r.anterior_quando) {
    desc.textContent = t("rede.mudancas_primeira");
    lista.replaceChildren();
    lista.hidden = true;
    return;
  }
  lista.hidden = false;
  desc.textContent = t("rede.mudancas_desc").replace("{data}", redeData(r.anterior_quando));
  if (!r.mudancas.length) {
    lista.replaceChildren(el("li", {}, el("p", {}, t("rede.sem_mudancas"))));
    return;
  }
  lista.replaceChildren(...r.mudancas.map((m) => {
    const quem = [m.camera_nome, m.fabricante].filter(Boolean).join(" · ") || (m.mac || "");
    let texto;
    if (m.tipo === "mudou_ip") {
      texto = el("span", { class: "mono" }, m.de, el("span", { class: "rede-seta" }, "→"), m.para);
    } else {
      texto = el("span", { class: "mono" }, m.ip);
    }
    return el("li", {},
      el("div", {}, el("div", { class: "actions" }, el("span", { class: "rede-mud " + m.tipo }, t("rede.mud." + m.tipo)), texto),
        quem ? el("p", {}, quem) : null),
      m.mac && m.tipo !== "mudou_ip" ? el("span", { class: "tiny mono" }, m.mac) : null);
  }));
}

function redeRenderHistorico(hist) {
  const lista = $("#rede-historico");
  if (!hist.length) {
    lista.replaceChildren(el("li", {}, el("p", {}, t("rede.sem_historico"))));
    return;
  }
  lista.replaceChildren(...hist.map((h, i) => el("li", {},
    el("div", {}, el("strong", {}, redeData(h.quando)), i === 0 ? el("span", { class: "rede-marca" }, t("rede.atual")) : null),
    el("span", { class: "tiny" }, `${h.dispositivos} ${t("rede.aparelhos")}`))));
}

async function redeRender() {
  let r;
  try { r = await api("/api/rede/foto"); } catch (_) { return; }
  const btn = $("#rede-foto");
  busy(btn, r.rodando, "rede.fotografando");
  const bloco = $("#rede-prog-bloco");
  bloco.hidden = !r.rodando;
  if (r.rodando) {
    const pct = Math.min(100, Math.round((r.progresso / r.total) * 100));
    $("#rede-prog-bar").style.width = pct + "%";
    $("#rede-prog").setAttribute("aria-valuenow", String(pct));
    $("#rede-prog-txt").textContent = t("rede.progresso").replace("{pct}", String(pct));
  }
  const erro = $("#rede-erro");
  erro.hidden = !r.erro;
  erro.textContent = r.erro || "";
  // Enquanto fotografa, mantém a foto anterior na tela.
  if (!r.rodando || !redeUltima) {
    redeUltima = r.ultima;
    redeRenderInventario(r.ultima);
    redeRenderMudancas(r);
    redeRenderHistorico(r.historico);
  }
  if (r.rodando && redeTimer == null) {
    redeTimer = setInterval(redeRender, 800);
  } else if (!r.rodando && redeTimer != null) {
    clearInterval(redeTimer);
    redeTimer = null;
  }
}

async function redeFoto() {
  try {
    await api("/api/rede/foto", {});
  } catch (e) {
    const erro = $("#rede-erro");
    erro.hidden = false;
    erro.textContent = e.message;
  }
  redeRender();
}

function redeAtivar(ativo) {
  if (ativo) {
    redeRender();
  } else if (redeTimer != null && $("#rede").hidden) {
    // Sai da tela com a foto em andamento: para o polling; volta ao entrar.
    clearInterval(redeTimer);
    redeTimer = null;
  }
}

function iniciarRede() {
  $("#rede-foto").onclick = redeFoto;
}

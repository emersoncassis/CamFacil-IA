// CamFácil IA — tela Rede (SDD-05). Usa t(), $, el(), api() e tempoRelativo() de app.js.
"use strict";

let redeTimer = null;
let redeUltima = null;
let redeDiagTimer = null;

/// t() com {chaves} preenchidas.
function tp(chave, params) {
  let s = t(chave);
  for (const [k, v] of Object.entries(params || {})) s = s.split("{" + k + "}").join(String(v));
  return s;
}

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
    el("div", { class: "actions" },
      el("span", { class: "tiny" }, `${h.dispositivos} ${t("rede.aparelhos")}`),
      el("a", { class: "btn sm", href: `/api/rede/diagrama.svg?foto=${h.id}&baixar=true`, download: "" }, t("rede.diagrama.historico"))))));
}

// ---------- Diagrama unifilar ----------
let redeDiagramaDe = null; // `quando` da foto desenhada

async function redeRenderDiagrama(foto) {
  const caixa = $("#rede-diagrama");
  const baixar = $("#rede-diagrama-baixar");
  if (!foto) {
    redeDiagramaDe = null;
    baixar.hidden = true;
    caixa.replaceChildren(el("p", { class: "hint" }, t("rede.diagrama.sem_foto")));
    return;
  }
  if (redeDiagramaDe === foto.quando) return;
  try {
    const r = await fetch("/api/rede/diagrama.svg");
    if (!r.ok) throw new Error(r.statusText);
    // SVG do próprio servidor, com textos já escapados.
    caixa.innerHTML = await r.text();
    redeDiagramaDe = foto.quando;
    baixar.hidden = false;
  } catch (_) { /* tenta de novo na próxima renderização */ }
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
    redeRenderDiagrama(r.ultima);
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

// ---------- Diagnóstico e troca de IP ----------

function redeDiagMsg(texto, tipo) {
  const m = $("#rede-diag-msg");
  m.textContent = texto || "";
  m.className = "msg " + (tipo || "");
}

function redeAlcance(d) {
  if (!d.alcance) return el("span", { class: "tag off" }, t("rede.alcance_nao"));
  const tag = el("span", { class: "tag " + (d.perda ? "warn" : "ok") }, tp("rede.alcance_ms", { ms: d.latencia_ms }));
  return d.perda ? el("span", {}, tag, el("span", { class: "tiny" }, " " + tp("rede.alcance_perda", { perda: d.perda }))) : tag;
}

function redeVideo(e) {
  const [ok, texto] = situacaoCam(e);
  return el("span", { class: "tag " + (ok ? "ok" : "off") }, texto);
}

async function redeAtualizarIp(m) {
  const p = { nome: m.nome, de: m.de, para: m.para };
  if (!confirm(tp("rede.confirma_atualizar", p))) return;
  try {
    await api(`/api/rede/cameras/${m.camera_id}/atualizar`, { para: m.para });
    redeDiagMsg(tp("rede.atualizado", p), "ok");
  } catch (e) {
    redeDiagMsg(e.message, "err");
  }
  redeDiagRender();
}

function redeRenderIpMudou(mudancas) {
  const lista = $("#rede-ip-mudou");
  lista.hidden = !mudancas.length;
  lista.replaceChildren(...mudancas.map((m) => {
    const btn = el("button", { class: "btn sm primary", type: "button" }, tp("rede.atualizar_btn", { para: m.para }));
    btn.onclick = () => redeAtualizarIp(m);
    return el("li", {},
      el("div", {}, el("strong", { class: "rede-mud mudou_ip" }, tp("rede.ip_mudou", m)),
        el("p", { class: "tiny mono" }, tp("rede.ip_mudou_mac", { mac: m.mac }))),
      btn);
  }));
}

async function redeDiagRender() {
  let r;
  try { r = await api("/api/rede/diagnostico"); } catch (_) { return; }
  let estado = {};
  try { ({ cameras: estado } = await api("/api/estado")); } catch (_) { /* segue sem vídeo */ }
  busy($("#rede-diag-btn"), r.rodando, "rede.diagnosticando");
  $("#rede-diag-btn").disabled = r.rodando || !r.cameras_rtsp;
  redeRenderIpMudou(r.mudancas);
  const corpo = $("#rede-diag");
  const vazio = (k) => corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "5" }, t(k))));
  if (!r.cameras_rtsp) vazio("rede.diag_sem_cameras");
  else if (!r.resultado.length) vazio(r.rodando ? "rede.diagnosticando" : "rede.diag_nunca");
  else {
    corpo.replaceChildren(...r.resultado.map((d) => el("tr", {},
      el("td", {}, d.nome),
      el("td", { class: "mono" }, `${d.host}:${d.porta}`, d.mac ? el("div", { class: "tiny" }, d.mac) : null),
      el("td", {}, redeAlcance(d)),
      el("td", {}, redeVideo(estado[String(d.camera_id)])),
      el("td", {}, el("ul", { class: "rede-dicas" }, ...d.dicas.map((x) => el("li", { class: x.codigo }, tp("rede.dica." + x.codigo, x.params))))))));
  }
  if (r.quando && !$("#rede-diag-msg").classList.contains("ok")) redeDiagMsg(tp("rede.diag_quando", { data: redeData(r.quando) }));
  if (r.rodando && redeDiagTimer == null) redeDiagTimer = setInterval(redeDiagRender, 1000);
  else if (!r.rodando && redeDiagTimer != null) { clearInterval(redeDiagTimer); redeDiagTimer = null; }
}

async function redeDiagnosticar() {
  redeDiagMsg("");
  try { await api("/api/rede/diagnostico", {}); } catch (e) { redeDiagMsg(e.message, "err"); }
  redeDiagRender();
}

/// Linha no bloco de saúde do Início quando alguma câmera trocou de IP.
async function redeResumoInicio() {
  const nota = $("#rede-resumo");
  if (!nota) return;
  try {
    const r = await api("/api/rede/resumo");
    nota.hidden = !r.mudancas;
    nota.querySelector("span").textContent = tp("rede.resumo_mudou", { n: r.mudancas });
  } catch (_) { nota.hidden = true; }
}

function redeAtivar(ativo) {
  if (ativo) {
    redeRender();
    redeDiagRender();
  } else if ($("#rede").hidden) {
    // Sai da tela com algo em andamento: para o polling; volta ao entrar.
    clearInterval(redeTimer);
    redeTimer = null;
    clearInterval(redeDiagTimer);
    redeDiagTimer = null;
  }
}

function iniciarRede() {
  $("#rede-foto").onclick = redeFoto;
  $("#rede-diag-btn").onclick = redeDiagnosticar;
}

// CamFácil IA — aba Segurança (ADR 0017): PIN do acesso pela rede, pedidos
// para autorizar (código conferido dos dois lados), aparelhos liberados,
// registro, IA conectada (MCP) e LGPD. Usa api, el, t e $ do app.js. JS puro.
"use strict";

let segTimer = null;
let segBadgeTimer = null;
let segPinVisivel = false;
let segUltimo = null;

function segData(unix) {
  return new Date(unix * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

function segMsg(sel, texto, tipo) {
  const m = $(sel);
  if (!m) return;
  m.textContent = texto || "";
  m.className = "msg " + (tipo || "");
}

function segMostrarPin() {
  const pin = segUltimo && segUltimo.pin;
  $("#seg-pin").textContent = pin ? (segPinVisivel ? pin.replace(/(\d{3})(\d{3,5})/, "$1 $2") : "••• •••") : "—";
  $("#seg-pin-ver").querySelector("span").textContent = t(segPinVisivel ? "seg.esconder" : "seg.ver");
}

function segPedidos(lista) {
  const ul = $("#seg-pedidos");
  ul.replaceChildren();
  if (!lista.length) { ul.append(el("li", { class: "vazio" }, t("seg.sem_pedidos"))); return; }
  for (const p of lista) {
    const sim = el("button", { class: "btn sm primary", type: "button" }, t("seg.autorizar"));
    const nao = el("button", { class: "btn sm", type: "button" }, t("seg.recusar"));
    const decidir = async (aprovar) => {
      sim.disabled = nao.disabled = true;
      try {
        await api(`/api/acesso/pedidos/${encodeURIComponent(p.id)}`, { aprovar });
        segMsg("#seg-msg", t(aprovar ? "seg.autorizado" : "seg.recusado"), "ok");
      } catch (e) { segMsg("#seg-msg", e.message, "err"); }
      segRender();
    };
    sim.onclick = () => decidir(true);
    nao.onclick = () => decidir(false);
    ul.append(el("li", { class: "seg-pedido" },
      el("div", {},
        el("strong", {}, p.nome),
        el("span", { class: "tiny" }, ` · ${p.ip} · ${t("seg.vence_em").replace("{s}", p.expira_s)}`)),
      el("code", { class: "mono seg-codigo" }, p.codigo.replace(/(\d{3})(\d{3})/, "$1 $2")),
      el("div", { class: "actions" }, nao, sim)));
  }
}

function segAparelhos(lista) {
  const ul = $("#seg-aparelhos");
  ul.replaceChildren();
  $("#seg-desconectar").hidden = !lista.length;
  if (!lista.length) { ul.append(el("li", { class: "vazio" }, t("seg.sem_aparelhos"))); return; }
  for (const a of lista) {
    const bloq = el("button", { class: "btn sm", type: "button" }, t(a.bloqueado ? "seg.liberar" : "seg.bloquear"));
    const rem = el("button", { class: "btn sm perigo", type: "button" }, t("seg.remover"));
    bloq.onclick = async () => {
      try { await api(`/api/acesso/aparelhos/${encodeURIComponent(a.id)}`, { bloqueado: !a.bloqueado }); } catch (e) { segMsg("#seg-ap-msg", e.message, "err"); }
      segRender();
    };
    rem.onclick = async () => {
      if (!confirm(t("seg.remover_confirmar").replace("{n}", a.nome))) return;
      try { await api(`/api/acesso/aparelhos/${encodeURIComponent(a.id)}`, undefined, "DELETE"); } catch (e) { segMsg("#seg-ap-msg", e.message, "err"); }
      segRender();
    };
    const metodo = t(a.metodo === "pin" ? "seg.metodo_pin" : "seg.metodo_confirmacao");
    ul.append(el("li", { class: a.bloqueado ? "seg-aparelho bloqueado" : "seg-aparelho" },
      el("div", {},
        el("strong", {}, a.nome + (a.bloqueado ? ` · ${t("seg.bloqueado")}` : "")),
        el("span", { class: "tiny" }, ` · ${metodo} · ${a.ip} · ${t("seg.ultimo_uso")} ${segData(a.ultimo_uso)} · ${t("seg.vale_ate")} ${segData(a.expira_em)}`)),
      el("div", { class: "actions" }, bloq, rem)));
  }
}

async function segRegistro() {
  let r;
  try { r = await api("/api/acesso/registro"); } catch (_) { return; }
  const ul = $("#seg-registro");
  ul.replaceChildren();
  if (!r.eventos.length) { ul.append(el("li", { class: "vazio" }, t("seg.sem_registro"))); return; }
  for (const e of r.eventos.slice(0, 30)) {
    const ruim = e.tipo.endsWith("negado");
    ul.append(el("li", { class: ruim ? "ev ev-alerta" : "ev" },
      el("span", { class: "tiny mono" }, segData(e.quando)),
      el("span", {}, ` ${t("seg.tipo_" + e.tipo)} · ${e.detalhe}`)));
  }
}

async function segIa() {
  let m;
  try { m = await api("/api/mcp"); } catch (_) { return; }
  $("#seg-ia-estado").textContent = t(m.ligado ? "seg.ia_ligado" : "seg.ia_desligado");
  $("#seg-ia-escrita").textContent = t(m.escrita ? "seg.ia_escrita_sim" : "seg.ia_escrita_nao");
  $("#seg-ia-limite").textContent = m.limite_por_minuto ? t("seg.ia_limite_valor").replace("{n}", m.limite_por_minuto) : "—";
}

function segEnderecos(lista) {
  const sel = $("#seg-endereco");
  const antes = sel.value;
  sel.replaceChildren(...lista.map((e, i) => el("option", { value: String(i) }, `${e.rotulo} · ${e.url}`)));
  if (antes && Number(antes) < lista.length) sel.value = antes;
  const tem = lista.length > 0;
  $("#seg-sem-rede").hidden = tem;
  sel.hidden = !tem;
  const img = $("#seg-qr");
  img.hidden = !tem;
  if (tem) {
    const i = Number(sel.value || 0);
    const src = `/api/acesso/qr.svg?e=${i}&v=${encodeURIComponent(lista[i].url)}`;
    if (img.getAttribute("src") !== src) img.src = src;
    img.alt = t("seg.qr_alt");
    $("#seg-qr-url").textContent = `${lista[i].url}/entrar`;
  } else {
    $("#seg-qr-url").textContent = "";
  }
}

async function segRender() {
  let r;
  try { r = await api("/api/acesso"); } catch (e) { segMsg("#seg-msg", e.message, "err"); return; }
  const local = !!r.local;
  document.querySelectorAll("#seguranca .seg-local").forEach((b) => { b.hidden = !local; });
  $("#seg-remoto").hidden = local;
  if (!local) {
    $("#seg-remoto-nome").textContent = r.aparelho ? r.aparelho.nome : "—";
    $("#seg-remoto-validade").textContent = r.aparelho ? segData(r.aparelho.expira_em) : "—";
    segRegistro();
    return;
  }
  segUltimo = r;
  segMostrarPin();
  $("#seg-bloqueio").checked = !!r.bloqueado;
  if (r.travado_s) segMsg("#seg-msg", t("seg.travado").replace("{m}", Math.ceil(r.travado_s / 60)), "err");
  segPedidos(r.pedidos || []);
  segAparelhos(r.aparelhos || []);
  segEnderecos(r.enderecos || []);
  segBadge(r.pedidos ? r.pedidos.length : 0);
  segRegistro();
  segIa();
  if (typeof renderTermos === "function") renderTermos();
}

// Pedidos esperando aparecem no menu, em qualquer aba.
function segBadge(n) {
  const b = $("#seg-badge");
  if (!b) return;
  b.hidden = !n;
  b.textContent = n ? String(n) : "";
}

async function segBadgeAtualizar() {
  try {
    const r = await api("/api/acesso");
    if (r.local) segBadge((r.pedidos || []).length);
  } catch (_) { /* sem conexão: tenta depois */ }
}

function segurancaAtivar(ativo) {
  clearInterval(segTimer);
  segTimer = null;
  if (ativo) { segRender(); segTimer = setInterval(segRender, 3000); }
}

function iniciarSeguranca() {
  $("#seg-pin-ver").onclick = () => { segPinVisivel = !segPinVisivel; segMostrarPin(); };
  $("#seg-pin-novo").onclick = async () => {
    if (!confirm(t("seg.gerar_confirmar"))) return;
    try { await api("/api/acesso/pin", {}); segPinVisivel = true; segMsg("#seg-msg", t("seg.pin_trocado"), "ok"); } catch (e) { segMsg("#seg-msg", e.message, "err"); }
    segRender();
  };
  $("#seg-pin-form").onsubmit = async (e) => {
    e.preventDefault();
    const f = e.target;
    try {
      await api("/api/acesso/pin", { pin: f.pin.value.trim() });
      f.reset();
      segPinVisivel = true;
      segMsg("#seg-msg", t("seg.pin_trocado"), "ok");
    } catch (err) { segMsg("#seg-msg", t("seg.pin_recusado").replace("{m}", err.message), "err"); }
    segRender();
  };
  $("#seg-bloqueio").onchange = async (e) => {
    try { await api("/api/acesso/bloqueio", { bloqueado: e.target.checked }); } catch (err) { segMsg("#seg-msg", err.message, "err"); }
    segRender();
  };
  $("#seg-endereco").onchange = () => segUltimo && segEnderecos(segUltimo.enderecos || []);
  $("#seg-desconectar").onclick = async () => {
    if (!confirm(t("seg.desconectar_confirmar"))) return;
    try { await api("/api/acesso/desconectar", {}); } catch (e) { segMsg("#seg-ap-msg", e.message, "err"); }
    segRender();
  };
  $("#seg-sair").onclick = async () => {
    try { await api("/api/acesso/sair", {}); } catch (_) { /* segue */ }
    location.href = "/entrar";
  };
  $("#seg-ia-config").onclick = () => {
    showTab("integracoes");
    if (typeof irSub === "function") irSub("integracoes", "mcp", false);
  };
  segBadgeAtualizar();
  segBadgeTimer = setInterval(segBadgeAtualizar, 5000);
}

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
  for (const [k, v] of Object.entries(attrs)) {
    // Eventos como propriedade: a CSP do site bloqueia atributos on* em texto.
    if (k === "class") e.className = v; else if (k.startsWith("on") && typeof v === "function") e[k] = v; else e.setAttribute(k, v);
  }
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

// ---------- Zero rolagem: listas que paginam no espaço que sobra ----------
// Desenha tudo, conta quantos itens cabem na caixa (altura fixa, sem rolagem)
// e, se não couber, divide em páginas com ‹ › embaixo.
const redesenhos = new Set();
let redimTimer = 0;
addEventListener("resize", () => { clearTimeout(redimTimer); redimTimer = setTimeout(() => redesenhos.forEach((f) => f()), 120); });

function contarCabem(caixa, limite) {
  const fim = limite.getBoundingClientRect().bottom + 0.5;
  let n = 0;
  for (const f of caixa.children) { if (f.getBoundingClientRect().bottom <= fim) n++; else break; }
  return n;
}

function paginar({ caixa, limite = caixa, pager, itens, criar, estado = {}, chave }) {
  const desenhar = () => {
    if (!caixa.isConnected) { redesenhos.delete(desenhar); return; }
    if (!caixa.offsetParent && caixa.getClientRects().length === 0) return; // escondida: desenha quando abrir
    pager.hidden = true;
    caixa.replaceChildren(...itens.map(criar));
    if (contarCabem(caixa, limite) >= itens.length) { estado.pagina = 0; return; }
    // Mostra o paginador já com a altura final antes de contar de novo.
    pager.hidden = false;
    pager.replaceChildren(el("button", { class: "btn ghost sm", type: "button", disabled: "" }, icone("i-ant", "ico sm")), el("span", { class: "tiny mono" }, "1 de 9"));
    const cabe = Math.max(1, contarCabem(caixa, limite));
    const total = Math.ceil(itens.length / cabe);
    estado.pagina = Math.max(0, Math.min(estado.pagina || 0, total - 1));
    caixa.replaceChildren(...itens.slice(estado.pagina * cabe, (estado.pagina + 1) * cabe).map(criar));
    const ir = (d) => { estado.pagina += d; desenhar(); };
    const ant = el("button", { class: "btn ghost sm", type: "button", "aria-label": "Página anterior", onclick: () => ir(-1) }, icone("i-ant", "ico sm"));
    const prox = el("button", { class: "btn ghost sm", type: "button", "aria-label": "Próxima página", onclick: () => ir(1) }, icone("i-prox", "ico sm"));
    ant.disabled = estado.pagina === 0; prox.disabled = estado.pagina >= total - 1;
    pager.replaceChildren(ant, el("span", { class: "tiny mono" }, `${estado.pagina + 1} de ${total}`), prox);
  };
  if (chave) { for (const f of redesenhos) if (f.chave === chave) redesenhos.delete(f); desenhar.chave = chave; }
  redesenhos.add(desenhar);
  desenhar();
  return desenhar;
}

// ---------- Navegação do painel ----------
function mostrar(id) {
  if (typeof vozParar === "function") vozParar();
  if (typeof fecharLeitor === "function" && leitorStream) fecharLeitor();
  document.querySelectorAll("#app [role=tab][data-tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === id)));
  document.querySelectorAll("#app main > [role=tabpanel]").forEach((p) => { p.hidden = p.id !== id; });
  if (id === "agentes") { vista(vistaAtual === "detalhe" && !detalheId ? "lista" : vistaAtual); carregarAgentes(); }
  if (id === "dispositivos") carregarDispositivos();
}

// Dentro do Painel, uma vista por vez: lista de clientes, como parear, ao vivo, detalhe.
let vistaAtual = "lista";
let detalheId = null;
function vista(nome) {
  vistaAtual = nome;
  document.querySelectorAll("#agentes .vista").forEach((v) => { v.hidden = v.dataset.vista !== nome; });
  $("#como-parear").setAttribute("aria-pressed", String(nome === "parear"));
  $("#ver-ts").setAttribute("aria-pressed", String(nome === "ts"));
  if (nome !== "parear") fecharLeitor();
  if (nome !== "detalhe") detalheId = null;
  if (nome === "lista") redesenhos.forEach((f) => { if (f.chave === "clientes") f(); });
}

// ---------- Agentes ----------
let agentes = [];
const paginaClientes = { pagina: 0 };

async function carregarAgentes() {
  const caixa = $("#lista-agentes");
  try {
    ({ agentes } = await api("/api/agentes"));
    desenharClientes();
    if (detalheId != null) abrirDetalhe(detalheId);
  } catch (e) {
    caixa.replaceChildren(el("p", { class: "msg err" }, e.message));
  }
}

function desenharClientes() {
  const caixa = $("#lista-agentes");
  if (!agentes.length) {
    $("#lista-barra").hidden = true;
    caixa.replaceChildren(el("div", { class: "cliente-vazio" },
      el("h2", {}, "Nenhum cliente ainda"),
      el("p", { class: "hint" }, "Instale o CamFácil IA no computador do cliente e pareie com este painel. As câmeras dele aparecem aqui."),
      el("div", { class: "actions mt" },
        el("button", { class: "btn primary", type: "button", onclick: () => mostrar("comecar") }, "Instalar num cliente"),
        el("button", { class: "btn", type: "button", onclick: () => vista("parear") }, "Já instalei, parear"))));
    return;
  }
  const filtro = ($("#filtro").value || "").trim().toLowerCase();
  const itens = filtro ? agentes.filter((a) => (a.apelido_cliente || "").toLowerCase().includes(filtro)) : agentes;
  const barra = $("#lista-barra");
  barra.hidden = agentes.length < 2;
  if (!itens.length) { caixa.replaceChildren(el("p", { class: "hint" }, "Nenhum cliente com esse nome.")); $("#pager-clientes").hidden = true; return; }
  paginar({ caixa, pager: $("#pager-clientes"), itens, criar: cartaoCliente, estado: paginaClientes, chave: "clientes" });
  // Sem filtro e tudo coube: a barra de busca não precisa ficar.
  if (!filtro && $("#pager-clientes").hidden && agentes.length < 7) barra.hidden = true;
}

function resumoCameras(e) {
  const cams = (e && e.cameras) || [];
  const on = cams.filter((c) => c.online).length;
  return cams.length ? `${on} de ${cams.length} ${cams.length === 1 ? "câmera" : "câmeras"} no ar` : "Nenhuma câmera ligada";
}

function linkAoVivo(a, rotulo = "Ver ao vivo", cls = "btn primary sm") {
  const ts = a.online && a.estado && a.estado.tailscale && a.estado.tailscale.url;
  return ts ? el("a", { class: cls, href: `${ts}/ao-vivo`, target: "_blank", rel: "noopener" }, rotulo) : null;
}

function cartaoCliente(a) {
  const campo = el("input", { class: "cell-edit cliente-nome", value: a.apelido_cliente, placeholder: "Nome do cliente", "aria-label": "Nome do cliente", maxlength: "60" });
  campo.addEventListener("change", async () => {
    try { await api(`/api/agentes/${a.id}`, "PATCH", { apelido_cliente: campo.value }); a.apelido_cliente = campo.value; } catch (e) { campo.value = a.apelido_cliente; alert(e.message); }
  });
  const e = a.estado;
  const camsOff = e ? (e.cameras || []).filter((c) => !c.online).length : 0;
  const alerta = e && (e.alertas || [])[0];
  const ramPct = e && e.ram_total_mb ? (e.ram_usada_mb * 100) / e.ram_total_mb : 0;
  return el("article", { class: "cliente" + (a.online ? "" : " off"), "data-agente": a.id },
    el("header", {}, campo,
      el("span", { class: "state", title: `Impressão ${a.impressao} · pareado em ${data(a.pareado_em)}` }, el("span", { class: "dot " + (a.online ? "on" : "") }), a.online ? "On-line" : "Desconectado")),
    e ? el("p", { class: "cliente-resumo" + (camsOff ? " atencao" : "") }, resumoCameras(e), camsOff ? el("span", { class: "tiny" }, ` · ${camsOff} sem sinal`) : null)
      : el("p", { class: "tiny cliente-resumo" }, a.online ? "Conectado. O primeiro resumo chega em alguns segundos." : "Ainda sem resumo deste computador."),
    e ? el("div", { class: "cliente-uso" }, barraUso("CPU", e.cpu, `${Math.round(e.cpu || 0)}%`), barraUso("Memória", ramPct, `${Math.round(ramPct)}%`)) : el("div", { class: "cliente-uso" }),
    el("p", { class: "tiny cliente-alerta" }, alerta ? `Último alerta: ${alerta.camera || "Câmera"} · ${TIPO_ALERTA[alerta.tipo] || alerta.tipo} · ${quandoRel(alerta.quando)}` : e ? "Nenhum alerta ainda." : ""),
    el("footer", {},
      linkAoVivo(a),
      el("button", { class: "btn sm", type: "button", onclick: () => abrirDetalhe(a.id) }, "Detalhes"),
      el("span", { class: "tiny mono cliente-versao" }, (e && e.versao) || a.versao || "")));
}

function abrirDetalhe(id) {
  const a = agentes.find((x) => x.id === id);
  if (!a) { vista("lista"); return; }
  vista("detalhe");
  detalheId = id;
  $("#detalhe-nome").textContent = a.apelido_cliente || "Cliente";
  $("#detalhe-estado").replaceChildren(el("span", { class: "dot " + (a.online ? "on" : "") }), a.online ? "On-line" : "Desconectado");
  const corpo = $("#detalhe-corpo");
  const aba = corpo.firstElementChild ? corpo.firstElementChild.dataset.aba : "cams";
  corpo.replaceChildren(visaoAgente(a.estado, a.estado_em, a.online, aba));
  ligarPaginasDetalhe(corpo);
}

function ligarPaginasDetalhe(raiz) {
  raiz.querySelectorAll("ul[data-paginar]").forEach((ul) => {
    paginar({ caixa: ul, pager: ul.nextElementSibling, itens: ul._itens, criar: ul._criar, estado: ul._estado, chave: "detalhe-" + ul.dataset.paginar });
  });
}

// ---------- O que o agente está vendo ----------
function quandoRel(seg) {
  if (seg == null) return "";
  const d = Math.max(0, Math.round(Date.now() / 1000 - seg));
  if (d < 60) return "agora";
  if (d < 3600) return `há ${Math.round(d / 60)} min`;
  if (d < 86400) return `há ${Math.round(d / 3600)} h`;
  return `há ${Math.round(d / 86400)} d`;
}

function barraUso(rotulo, pct, texto) {
  const p = Math.max(0, Math.min(100, pct || 0));
  const nivel = p >= 85 ? "alto" : p >= 60 ? "medio" : "";
  return el("div", { class: "uso" }, el("span", { class: "tiny" }, rotulo),
    el("span", { class: "uso-barra " + nivel }, (() => { const i = el("i"); i.style.setProperty("--p", `${p}%`); return i; })()),
    el("span", { class: "mono tiny" }, texto));
}

const TIPO_ALERTA = { linha_cruzada: "cruzou a linha", zona_ocupada: "entrou na zona", movimento: "movimento", saude: "problema na câmera" };

// Lista que pagina dentro da coluna (câmeras e alertas podem ser muitos).
function listaPaginada(nome, itens, criar, vazio) {
  const ul = el("ul", { class: `${nome}-agente`, "data-paginar": nome });
  ul._itens = itens.length ? itens : [null];
  ul._criar = (x) => (x == null ? el("li", { class: "tiny" }, vazio) : criar(x));
  ul._estado = { pagina: 0 };
  return [ul, el("div", { class: "pager", hidden: "" })];
}

function visaoAgente(e, em, online, aba = "cams") {
  if (!e) return el("p", { class: "tiny vazio-agente" }, online
    ? "Conectado. O primeiro resumo chega em alguns segundos (atualize o programa no cliente se não aparecer)."
    : "Ainda sem resumo deste computador. Ele aparece quando o programa estiver aberto e com internet.");
  const cams = e.cameras || [];
  const on = cams.filter((c) => c.online).length;
  // Ao vivo pelo Tailscale: abre a página leve do próprio programa do cliente.
  const ts = online && e.tailscale && e.tailscale.url;
  const aoVivo = (q, rotulo, cls) => el("a", { class: cls, href: `${ts}/ao-vivo${q}`, target: "_blank", rel: "noopener" }, rotulo);
  const camLista = listaPaginada("cams", cams, (c) =>
    el("li", {}, el("span", { class: "dot " + (c.online ? "on" : "off") }),
      ts && c.id != null ? aoVivo(`?camera=${c.id}`, c.nome || "Câmera", "cam-link") : el("span", {}, c.nome || "Câmera"),
      el("span", { class: "tiny" }, c.online ? `${Math.round((c.fps ?? 0) * 10) / 10} q/s${c.regras ? ` · ${c.regras} ${c.regras === 1 ? "regra" : "regras"}` : ""}${c.saude ? ` · ${c.saude}` : ""}` : (c.erro || "sem sinal"))),
  "Nenhuma câmera ligada ainda.");
  const ramPct = e.ram_total_mb ? (e.ram_usada_mb * 100) / e.ram_total_mb : 0;
  const maquina = el("div", { class: "maq-agente" },
    barraUso("CPU", e.cpu, `${Math.round(e.cpu || 0)}%`),
    barraUso("Memória", ramPct, e.ram_total_mb ? `${(e.ram_usada_mb / 1024).toFixed(1)} de ${(e.ram_total_mb / 1024).toFixed(1)} GB` : "—"),
    el("p", { class: "tiny" }, [e.maquina, e.sistema, e.proprio_mb != null ? `CamFácil IA usando ${e.proprio_mb} MB` : null].filter(Boolean).join(" · ")),
    el("p", { class: "tiny" }, (e.modulos || []).length ? `IA ligada: ${e.modulos.join(", ")}` : "Nenhuma IA ligada"));
  const alertLista = listaPaginada("alertas", (e.alertas || []).slice(0, 20), (a) =>
    el("li", {}, el("span", {}, `${a.camera || "Câmera"}${a.regra ? ` · ${a.regra}` : ""}`),
      el("span", { class: "tiny" }, `${TIPO_ALERTA[a.tipo] || a.tipo} · ${quandoRel(a.quando)}`)),
  "Nenhum alerta ainda.");
  const atual = em ? `Atualizado ${quandoRel(Date.parse(em) / 1000)}` : "";
  const barraVivo = el("div", { class: "vivo-barra" }, ts
    ? aoVivo("", "Ver câmeras ao vivo", "btn primary")
    : el("p", { class: "tiny" }, online
      ? "Para ver ao vivo, ligue o Tailscale no computador do cliente e no seu celular (veja em Ao vivo no celular)."
      : "Ao vivo fica disponível quando o computador do cliente estiver ligado."),
    el("span", { class: "tiny" }, atual));
  // No celular, uma coluna por vez (abas); na tela larga, as três lado a lado.
  const caixa = el("div", { class: "agente-visao", "data-aba": aba });
  const abas = el("div", { class: "seg visao-abas", role: "tablist", "aria-label": "Detalhes do cliente" });
  for (const [id, rotulo] of [["cams", `Câmeras ${on}/${cams.length}`], ["maq", "Máquina"], ["alertas", "Alertas"]]) {
    const b = el("button", { type: "button", role: "tab", "aria-selected": String(id === aba), onclick: () => {
      caixa.dataset.aba = id;
      abas.querySelectorAll("button").forEach((x) => x.setAttribute("aria-selected", String(x === b)));
      ligarPaginasDetalhe(caixa);
    } }, rotulo);
    abas.append(b);
  }
  caixa.append(barraVivo, abas,
    el("section", { class: "s-cams" }, el("h3", {}, `Câmeras · ${on} de ${cams.length} no ar`), ...camLista),
    el("section", { class: "s-maq" }, el("h3", {}, "Máquina do cliente"), maquina),
    el("section", { class: "s-alertas" }, el("h3", {}, "Últimos alertas"), ...alertLista));
  return caixa;
}

// ---------- Dispositivos ----------
const paginaDisp = { pagina: 0 };
async function carregarDispositivos() {
  const corpo = $("#lista-dispositivos");
  const atual = await idbGet(`dispositivo:${eu.id}`);
  const { dispositivos } = await api("/api/dispositivos");
  const ativos = dispositivos.filter((d) => !d.revogado);
  if (!ativos.length) { $("#pager-disp").hidden = true; corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "4" }, "Nenhum dispositivo ativo."))); return; }
  const linha = (d) => {
    const btn = el("button", { class: "btn sm", type: "button" }, "Revogar");
    btn.onclick = async () => {
      if (!confirm(`Revogar "${d.apelido || "dispositivo"}"? Os agentes deixam de aceitar comandos dele.`)) return;
      await api(`/api/dispositivos/${d.id}`, "DELETE");
      carregarDispositivos();
    };
    const nome = d.apelido || "Dispositivo";
    return el("tr", {},
      el("td", {}, atual && atual.id === d.id ? `${nome} (este)` : nome),
      el("td", { class: "mono col-impressao" }, d.impressao),
      el("td", {}, data(d.criado_em)),
      el("td", { class: "right" }, btn));
  };
  paginar({ caixa: corpo, limite: $("#disp-wrap"), pager: $("#pager-disp"), itens: ativos, criar: linha, estado: paginaDisp, chave: "disp" });
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

// ---------- Alternativas ao QR: leitor no site e código digitado ----------
let leitorStream = null;

function irParaParear(codigo, impressao) {
  const c = (codigo || "").replace(/\s+/g, "");
  if (!/^[A-Za-z0-9_-]{20,}$/.test(c)) { msgLeitor("Código incompleto. Confira na tela do cliente.", "err"); return; }
  history.replaceState(null, "", `/parear#c=${encodeURIComponent(c)}${impressao ? `&f=${encodeURIComponent(impressao)}` : ""}`);
  fecharLeitor();
  telaParear();
}

function msgLeitor(t, tipo) { const m = $("#leitor-msg"); m.textContent = t; m.className = "msg " + (tipo || ""); }

function fecharLeitor() {
  if (leitorStream) { leitorStream.getTracks().forEach((t) => t.stop()); leitorStream = null; }
  const l = $("#leitor"); if (l) l.hidden = true;
  const p = $("#passos"); if (p) p.classList.remove("lendo-qr");
}

async function abrirLeitor() {
  $("#form-codigo").hidden = true;
  if (!("BarcodeDetector" in window)) {
    msgLeitor("Este navegador não lê QR pela página (comum no iPhone). Abra a câmera normal do celular ou use Digitar código.", "err");
    return;
  }
  try {
    const det = new BarcodeDetector({ formats: ["qr_code"] });
    leitorStream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "environment" } });
    const v = $("#leitor-video");
    v.srcObject = leitorStream;
    await v.play();
    $("#leitor").hidden = false;
    $("#passos").classList.add("lendo-qr"); // esconde os passos para a câmera caber na tela
    msgLeitor("Aponte para o QR na tela do cliente.");
    const procurar = async () => {
      if (!leitorStream) return;
      try {
        const [r] = await det.detect(v);
        if (r && r.rawValue.includes("/parear#")) {
          const p = new URLSearchParams(r.rawValue.split("#")[1]);
          irParaParear(p.get("c"), p.get("f"));
          return;
        }
      } catch (_) { /* quadro ruim: tenta o próximo */ }
      setTimeout(procurar, 250);
    };
    procurar();
  } catch (e) {
    fecharLeitor();
    msgLeitor("Não consegui abrir a câmera. Libere a permissão no navegador ou use Digitar código.", "err");
  }
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
      setTimeout(() => { history.replaceState(null, "", "/"); vistaAtual = "lista"; mostrar("agentes"); }, 1200);
    } else if (m.tipo === "pareamento_recusado") {
      msgPar("O pareamento foi recusado na tela do cliente.", "err");
    } else if (m.tipo === "estado") {
      const a = agentes.find((x) => x.id === m.agente_id);
      if (!a) return;
      a.estado = m.estado; a.estado_em = new Date().toISOString(); a.online = true;
      const card = document.querySelector(`#lista-agentes [data-agente="${m.agente_id}"]`);
      if (card && document.activeElement && card.contains(document.activeElement)) return; // não atrapalha quem está digitando o nome
      if (card) card.replaceWith(cartaoCliente(a));
      if (detalheId === a.id && vistaAtual === "detalhe") abrirDetalhe(a.id);
    } else if (["agente_online", "agente_offline", "despareado"].includes(m.tipo) && !$("#agentes").hidden) {
      carregarAgentes();
    }
  };
}

// ---------- Download do programa (Agente-Cam-Elo-IA embutido) ----------
// /baixar/<arquivo> manda para o arquivo com o número da versão no nome.
const REL = "/baixar/";
const SISTEMAS = {
  windows: { nome: "Windows 10 e 11", curto: "Windows", ico: "i-win", arquivo: "camfacil-windows-x86_64.exe", cmd: "irm https://cfia.ysafe.com.br/instalar.ps1 | iex", onde: "PowerShell" },
  linux: { nome: "Linux (PC ou mini PC)", curto: "Linux", ico: "i-tux", arquivo: "camfacil-linux-x86_64", cmd: "curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh", onde: "terminal" },
  rpi: { nome: "Raspberry Pi e TV Box (ARM 64)", curto: "Raspberry", ico: "i-rpi", arquivo: "camfacil-linux-arm64", cmd: "curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh", onde: "terminal" },
  android: { nome: "Android (celular e TV Box)", curto: "Android", ico: "i-phone", arquivo: "camfacil-android.apk" },
};

function sistemaDoVisitante() {
  const ua = navigator.userAgent;
  if (/Windows/.test(ua)) return "windows";
  if (/Android/.test(ua)) return "android";
  if (/aarch64|arm64|armv8|Raspbian/i.test(ua)) return "rpi";
  if (/Linux|X11/.test(ua) && !/Android/.test(ua)) return "linux";
  return "windows";
}

function icone(id, cls = "ico") {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg"); svg.setAttribute("class", cls);
  const use = document.createElementNS(ns, "use"); use.setAttribute("href", "#" + id); svg.append(use);
  return svg;
}

function caixaComando(texto) {
  const b = el("button", { class: "btn ghost sm", type: "button", "data-copiar": "" }, icone("i-copy", "ico sm"), el("span", {}, "Copiar"));
  return el("div", { class: "cmd" }, el("code", { class: "mono" }, texto), b);
}

// Seção que abre (acordeão). Com o mesmo `name`, abrir uma fecha as outras.
function acordeao(titulo, grupo, aberto, ...filhos) {
  const d = el("details", { class: "acord", name: grupo, "data-acord": titulo }, el("summary", {}, el("span", {}, titulo), icone("i-prox", "ico sm")), ...filhos);
  d.open = !!aberto;
  return d;
}

function montarDownload(alvo) {
  let atual = sistemaDoVisitante();
  const grupo = alvo.dataset.grupo || "dl";
  const corpo = el("div", { class: "dl-corpo" });
  const abas = el("div", { class: "seg", role: "radiogroup", "aria-label": "Sistema" });
  const desenhar = () => {
    const s = SISTEMAS[atual];
    if (atual === "android") {
      corpo.replaceChildren(
        el("div", { class: "actions" },
          el("a", { class: "btn primary grande", href: REL + s.arquivo, download: "" }, icone("i-down"), el("span", {}, "Baixar o app Android")),
          el("span", { class: "tiny" }, "Grátis · cerca de 8 MB · Android 7 ou mais novo")),
        acordeao("Passo a passo", grupo, true, el("ol", { class: "steps" },
          el("li", {}, "Baixe pelo celular ou pela TV Box e toque no arquivo."),
          el("li", {}, "Se o Android avisar, toque em ", el("strong", {}, "Configurações"), " e ative ", el("strong", {}, "Permitir desta fonte"), ". O app é gratuito e fora da Play Store."),
          el("li", {}, "Escolha ", el("strong", {}, "Este aparelho é o servidor"), " para ligar as câmeras nele, ou ", el("strong", {}, "Ver um servidor à distância"), " para ver de longe pelo Tailscale."))));
      return;
    }
    const passos = atual === "windows"
      ? [
          el("li", {}, "Clique em ", el("strong", {}, "Baixar para Windows"), " e abra o arquivo baixado."),
          el("li", {}, "Se aparecer a tela azul do Windows, clique em ", el("strong", {}, "Mais informações"), " e depois em ", el("strong", {}, "Executar assim mesmo"), ". É porque o programa é novo e gratuito, sem certificado pago."),
          el("li", {}, "O CamFácil IA abre no navegador, já na tela ", el("strong", {}, "Ligar câmera"), "."),
        ]
      : [
          el("li", {}, "Abra o terminal, cole o comando abaixo e aperte Enter."),
          el("li", {}, "Ele baixa o programa, instala o ffmpeg se faltar e abre o CamFácil IA no navegador."),
          el("li", {}, "Sem tela (servidor ou TV Box)? Rode ", el("code", { class: "mono" }, "camfacil --lan"), " e abra pelo celular no endereço que aparecer."),
        ];
    corpo.replaceChildren(
      el("div", { class: "actions" },
        el("a", { class: "btn primary grande", href: REL + s.arquivo, download: "" }, icone("i-down"), el("span", {}, atual === "windows" ? "Baixar para Windows" : `Baixar para ${s.curto}`)),
        el("span", { class: "tiny" }, "Grátis · cerca de 4 MB · sem cadastro")),
      acordeao("Passo a passo", grupo, true, el("ol", { class: "steps" }, ...passos)),
      acordeao(atual === "windows" ? "Instalar pelo PowerShell (com atalho)" : "Comando de instalação", grupo, false,
        el("p", { class: "tiny" }, atual === "windows" ? "Prefere instalar com atalho na área de trabalho? Cole no PowerShell:" : "Cole no terminal:"),
        caixaComando(s.cmd),
        el("p", { class: "tiny" }, "Para mostrar o vídeo, o programa usa o ffmpeg (gratuito). Se faltar, ele mesmo instala ou mostra como.")));
    ligarCopiar(corpo);
  };
  // Um nome de grupo só por seletor, para os botões se alternarem.
  const radio = "so-" + Math.random().toString(36).slice(2);
  for (const [id, s] of Object.entries(SISTEMAS)) {
    const input = el("input", { type: "radio", name: radio, value: id, "aria-label": s.nome });
    input.checked = id === atual;
    input.onchange = () => { atual = id; desenhar(); };
    abas.append(el("label", { title: s.nome }, input, el("span", {}, icone(s.ico, "ico sm"), el("b", { class: "longo" }, s.nome), el("b", { class: "curto" }, s.curto))));
  }
  alvo.replaceChildren(abas, corpo);
  desenhar();
}

function ligarCopiar(raiz = document) {
  raiz.querySelectorAll("[data-copiar]").forEach((b) => {
    b.onclick = async () => {
      const txt = b.parentElement.querySelector("code").textContent;
      try { await navigator.clipboard.writeText(txt); b.querySelector("span").textContent = "Copiado"; setTimeout(() => { b.querySelector("span").textContent = "Copiar"; }, 1600); } catch (_) { /* sem permissão */ }
    };
  });
}

// ---------- Narração na voz do Emerson ----------
// Em vez de rolar, a narração abre a seção de que está falando.
const voz = new Audio();
let vozFila = [];
let vozBotao = null;
let vozGuia = null; // (trecho) => abre a seção certa

function vozTopo() {
  const t = $("#voz-topo");
  if (!t) return;
  t.hidden = !vozBotao || !!vozBotao.offsetParent;
}

function vozParar() {
  voz.pause(); vozFila = []; vozGuia = null;
  if (vozBotao) { vozBotao.querySelector("use").setAttribute("href", "#i-play"); vozBotao.classList.remove("tocando"); vozBotao = null; }
  vozTopo();
}

function vozProxima() {
  const f = vozFila.shift();
  if (!f) { vozParar(); return; }
  if (vozGuia) vozGuia(f);
  voz.src = `/audio/${f}.ogg`;
  voz.play().catch(vozParar);
  vozTopo();
}
voz.onended = vozProxima;

async function prepararNarracao(guia) {
  const caixas = [...document.querySelectorAll("[data-narracao]")];
  if (!caixas.length) return;
  // Só mostra o botão se as gravações existem neste servidor.
  const ok = await fetch("/audio/site-01-boas-vindas.ogg", { method: "HEAD" }).then((r) => r.ok).catch(() => false);
  if (!ok) return;
  for (const c of caixas) {
    c.hidden = false;
    const b = c.querySelector("button");
    b.onclick = () => {
      if (vozBotao === b) { vozParar(); return; }
      vozParar();
      vozBotao = b; b.classList.add("tocando"); b.querySelector("use").setAttribute("href", "#i-pause");
      vozFila = c.dataset.narracao.split(",");
      vozGuia = vozFila.length > 1 ? guia : null;
      vozProxima();
    };
  }
}

// ---------- Site do visitante: uma seção por vez ----------
const SECOES = ["inicio", "baixar", "ferramentas", "dentro", "formacao", "quem", "painel", "apoiar"];
// Âncoras antigas e de fora (/#baixar, /#site-t, /#ferramentas…) abrem a seção certa.
const ANCORAS = { "baixar-t": "baixar", como: "baixar", "como-t": "baixar", "ferr-t": "ferramentas", "form-t": "formacao", "quem-t": "quem", "site-t": "painel", "painel-t": "painel", proprio: "painel", entrar: "painel", "dentro-t": "dentro", "por-dentro": "dentro", codigo: "dentro" };
const largo = matchMedia("(min-width: 1000px) and (min-height: 560px)");

function secaoDaAncora(h) {
  const id = (h || "").replace(/^#/, "");
  if (SECOES.includes(id)) return { secao: id };
  if (ANCORAS[id]) return { secao: ANCORAS[id], detalhe: id };
  return null;
}

function abrirSecao(nome, { detalhe, focar } = {}) {
  if (!SECOES.includes(nome)) nome = "inicio";
  document.querySelectorAll("#entrar [data-secao]").forEach((b) => {
    const sel = b.dataset.secao === nome;
    b.setAttribute("aria-selected", String(sel));
    b.tabIndex = sel ? 0 : -1;
    if (sel && focar) b.focus();
  });
  document.querySelectorAll("#entrar .secao").forEach((s) => { s.hidden = s.id !== nome; });
  // O botão "Baixar grátis" do topo é o único caminho do menu para o download: fica marcado na tela de download.
  document.querySelectorAll("#entrar .cta-baixar").forEach((c) => { if (nome === "baixar") c.setAttribute("aria-current", "page"); else c.removeAttribute("aria-current"); });
  if (["como", "como-t"].includes(detalhe)) $("#como").open = true;
  if (["proprio", "painel-t"].includes(detalhe)) $("#proprio").open = true;
  const url = nome === "inicio" ? location.pathname + location.search : `#${nome}`;
  history.replaceState(null, "", url);
  menuFechar();
  vozTopo();
}

// ---------- Menu: abas quando cabem; quando não cabem, vira menu ----------
// Mede de verdade (texto, zoom da tela grande, idioma) em vez de chutar
// uma largura: se as abas não cabem na barra, o site passa a usar o botão
// de menu e nunca corta o nome de uma seção.
function menuFechar(focarBotao) {
  const b = document.querySelector("#entrar .menu-btn");
  const site = $("#entrar");
  if (!b || !site.classList.contains("menu-aberto")) return;
  site.classList.remove("menu-aberto");
  b.setAttribute("aria-expanded", "false");
  b.setAttribute("aria-label", "Abrir o menu");
  b.querySelector("use").setAttribute("href", "#i-menu");
  if (focarBotao) b.focus();
}

function ajustarMenu() {
  const site = $("#entrar");
  const nav = $("#menu-secoes");
  if (!site || !nav || site.hidden) return;
  // 1º tenta as abas com ícone; 2º sem ícone; 3º vira menu.
  site.classList.remove("menu-compacto", "abas-sem-icone");
  const lista = nav.querySelector("ul");
  const estoura = () => lista.scrollWidth > nav.clientWidth + 1;
  if (matchMedia("(max-width: 760px)").matches) { site.classList.add("menu-compacto"); return; }
  if (estoura()) site.classList.add("abas-sem-icone");
  const naoCabe = estoura();
  site.classList.toggle("menu-compacto", naoCabe);
  if (!naoCabe) menuFechar();
}

function ligarMenu() {
  const site = $("#entrar");
  const b = site.querySelector(".menu-btn");
  b.onclick = () => {
    const abrir = !site.classList.contains("menu-aberto");
    if (!abrir) { menuFechar(); return; }
    site.classList.add("menu-aberto");
    b.setAttribute("aria-expanded", "true");
    b.setAttribute("aria-label", "Fechar o menu");
    b.querySelector("use").setAttribute("href", "#i-x");
    const sel = site.querySelector('#menu-secoes [aria-selected="true"]') || site.querySelector("#menu-secoes [data-secao]");
    if (sel) sel.focus();
  };
  addEventListener("keydown", (ev) => { if (ev.key === "Escape") menuFechar(true); });
  document.addEventListener("click", (ev) => {
    if (!site.classList.contains("menu-aberto")) return;
    if (ev.target.closest("#menu-secoes, .menu-btn")) return;
    menuFechar();
  });
  ajustarMenu();
  new ResizeObserver(ajustarMenu).observe(site.querySelector(".topo-site") ? site : document.body);
  addEventListener("resize", ajustarMenu);
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(ajustarMenu);
}

// ---------- Título: frases que se digitam (as mesmas do README) ----------
function ligarDigitar() {
  const alvo = document.querySelector("#entrar .digita-txt");
  if (!alvo || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const frases = [
    "IA de vídeo em qualquer máquina, com um comando.",
    "Aprenda IA e rede IP fazendo.",
    "Valide no cliente. Venda a câmera com IA.",
    "Feito por instalador, para instalador.",
  ];
  let f = 0, n = frases[0].length, apagando = true;
  const passo = () => {
    const frase = frases[f];
    if (apagando) {
      n -= 1;
      if (n <= 0) { apagando = false; f = (f + 1) % frases.length; n = 0; }
    } else {
      n += 1;
      if (n >= frases[f].length) { apagando = true; alvo.textContent = frases[f]; setTimeout(passo, 2600); return; }
    }
    alvo.textContent = (apagando ? frase : frases[f]).slice(0, Math.max(n, 0));
    setTimeout(passo, apagando ? 28 : 55);
  };
  setTimeout(passo, 3200);
}

// ---------- Por dentro: cada tópico mostra o seu diagrama ----------
function ligarDentro() {
  const img = $("#dentro-img");
  if (!img) return;
  const link = img.closest("a");
  const legenda = $("#dentro-legenda");
  const padrao = { src: img.getAttribute("src"), alt: img.alt, href: link.getAttribute("href"), legenda: legenda.textContent, w: 1200, h: 320 };
  const mostrar = (v) => {
    img.src = v.src; img.alt = v.alt; link.href = v.href; legenda.textContent = v.legenda;
    img.width = v.w; img.height = v.h;
  };
  document.querySelectorAll("#dentro details[name='dentro']").forEach((d) => {
    d.addEventListener("toggle", () => {
      const aberto = [...document.querySelectorAll("#dentro details[name='dentro']")].find((x) => x.open && x.dataset.visual);
      if (!aberto) { mostrar(padrao); return; }
      mostrar({ src: aberto.dataset.visual, alt: aberto.dataset.alt || "", href: aberto.dataset.visual,
        legenda: "Clique para ver o diagrama em tamanho grande.", w: 1400, h: 700 });
    });
  });
}

// "Como funciona": na tela larga fica sempre aberto ao lado do download;
// na estreita vira mais uma seção do acordeão.
function ajustarComo() {
  const d = $("#como");
  if (!d) return;
  if (largo.matches) { d.removeAttribute("name"); d.open = true; d.classList.add("fixo"); $("#proprio").open = true; }
  else { d.setAttribute("name", "baixar"); d.classList.remove("fixo"); if (d.open) d.open = false; }
}

function ligarFerramentas() {
  const botoes = [...document.querySelectorAll(".ferr-lista [data-ferr]")];
  const escolher = (id, focar) => {
    botoes.forEach((b) => { const s = b.dataset.ferr === id; b.setAttribute("aria-selected", String(s)); b.tabIndex = s ? 0 : -1; if (s && focar) b.focus(); });
    document.querySelectorAll(".ferr-detalhe [data-ferr]").forEach((a) => { a.hidden = a.dataset.ferr !== id; });
  };
  botoes.forEach((b, i) => {
    b.onclick = () => escolher(b.dataset.ferr);
    b.onkeydown = (ev) => {
      const d = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[ev.key];
      if (!d) return;
      ev.preventDefault();
      escolher(botoes[(i + d + botoes.length) % botoes.length].dataset.ferr, true);
    };
  });
  escolher(botoes[0].dataset.ferr);
}

function iniciarVisitante(erro) {
  $("#entrar").hidden = false;
  if (erro) { $("#entrar-erro").textContent = "Login cancelado. Tente de novo."; }
  document.querySelectorAll("#entrar [data-baixar]").forEach(montarDownload);
  ligarCopiar($("#entrar"));
  ligarFerramentas();
  ligarMenu();
  ligarDigitar();
  ligarDentro();
  ajustarComo();
  largo.addEventListener("change", ajustarComo);
  $("#como").querySelector("summary").addEventListener("click", (ev) => { if ($("#como").classList.contains("fixo")) ev.preventDefault(); });

  const abas = [...document.querySelectorAll("#entrar [data-secao]")];
  abas.forEach((b, i) => {
    b.onclick = () => abrirSecao(b.dataset.secao);
    b.onkeydown = (ev) => {
      const d = { ArrowRight: 1, ArrowLeft: -1 }[ev.key];
      if (!d) return;
      ev.preventDefault();
      abrirSecao(abas[(i + d + abas.length) % abas.length].dataset.secao, { focar: true });
    };
  });
  // Links internos (#baixar, #painel…) trocam de seção em vez de rolar.
  $("#entrar").addEventListener("click", (ev) => {
    const a = ev.target.closest("a[href^='#']");
    if (!a) return;
    const alvo = secaoDaAncora(a.getAttribute("href"));
    if (!alvo) return;
    ev.preventDefault();
    abrirSecao(alvo.secao, alvo);
  });
  addEventListener("hashchange", () => { const a = secaoDaAncora(location.hash); if (a) abrirSecao(a.secao, a); });
  const ini = secaoDaAncora(location.hash) || (erro ? { secao: "painel" } : { secao: "inicio" });
  abrirSecao(ini.secao, ini);

  $("#voz-topo").onclick = vozParar;
  // Boas-vindas narradas: cada trecho abre a seção de que fala.
  prepararNarracao((trecho) => {
    const mapa = {
      "site-01-boas-vindas": ["inicio"],
      "site-02-por-que": ["quem"],
      "site-03-passo-a-passo": ["baixar", "como"],
      "site-04-download": ["baixar"],
      "site-05-painel": ["painel", "proprio"],
    }[trecho];
    if (!mapa) return;
    abrirSecao(mapa[0], { detalhe: mapa[1] });
    if (trecho === "site-04-download") {
      const passo = document.querySelector("#baixar [data-acord='Passo a passo']");
      if (passo) passo.open = true;
    }
  });
}

// ---------- Instalar: um passo por vez ----------
function irPasso(n) {
  document.querySelectorAll("#comecar [data-passo]").forEach((x) => {
    if (x.getAttribute("role") === "tab") x.setAttribute("aria-selected", String(x.dataset.passo === String(n)));
    else x.hidden = x.dataset.passo !== String(n);
  });
}

// ---------- Início ----------
async function main() {
  const erro = new URLSearchParams(location.search).get("erro");
  try {
    eu = await api("/api/eu");
  } catch (_) {
    if (location.pathname === "/parear") sessionStorage.setItem("cfia-voltar", location.pathname + location.hash);
    iniciarVisitante(erro);
    return;
  }
  const voltar = sessionStorage.getItem("cfia-voltar");
  if (voltar) { sessionStorage.removeItem("cfia-voltar"); history.replaceState(null, "", voltar); }

  $("#app").hidden = false;
  $("#eu-email").textContent = eu.email;
  document.querySelectorAll("#app [role=tab][data-tab]").forEach((b) => { b.onclick = () => { history.replaceState(null, "", "/"); mostrar(b.dataset.tab); }; });
  $("#como-parear").onclick = () => vista(vistaAtual === "parear" ? "lista" : "parear");
  $("#ver-ts").onclick = () => vista(vistaAtual === "ts" ? "lista" : "ts");
  document.querySelectorAll("#agentes [data-voltar]").forEach((b) => { b.onclick = () => vista("lista"); });
  $("#filtro").oninput = () => { paginaClientes.pagina = 0; desenharClientes(); };
  $("#ler-qr").onclick = abrirLeitor;
  $("#leitor-fechar").onclick = () => { fecharLeitor(); msgLeitor(""); };
  $("#digitar-codigo").onclick = () => { fecharLeitor(); msgLeitor(""); $("#form-codigo").hidden = false; $("#passos").classList.add("lendo-qr"); $("#form-codigo").codigo.focus(); };
  $("#form-codigo").onsubmit = (ev) => { ev.preventDefault(); irParaParear(ev.target.codigo.value, null); };
  $("#sair").onclick = async () => { await api("/api/auth/sair", "POST"); location.href = "/"; };
  document.querySelectorAll("#comecar [role=tab][data-passo]").forEach((b) => { b.onclick = () => irPasso(b.dataset.passo); });
  document.querySelectorAll("#comecar [data-ir-passo]").forEach((b) => { b.onclick = () => irPasso(b.dataset.irPasso); });
  document.querySelectorAll("#comecar [data-abrir-parear]").forEach((b) => { b.onclick = () => { history.replaceState(null, "", "/"); mostrar("agentes"); vista("parear"); }; });

  document.querySelectorAll("#app [data-baixar]").forEach(montarDownload);
  // Passo a passo narrado: cada trecho abre o passo de que fala.
  prepararNarracao((trecho) => { const n = { "site-03-passo-a-passo": 2, "site-04-download": 1, "site-05-painel": 3 }[trecho]; if (n) irPasso(n); });
  garantirDispositivo().catch(() => {});
  conectarPainel();
  if (location.pathname === "/parear") { telaParear(); return; }
  // Logado, vai direto ao painel. Sem cliente ainda, o próprio painel mostra o primeiro passo.
  mostrar("agentes");
}

main();

// Apoiar: "Entrar no mural" é um bloco fechado; o formulário só aparece no clique.
(() => {
  const abrir = document.getElementById("abrir-mural");
  const form = document.getElementById("form-mural");
  const fechar = document.getElementById("fechar-mural");
  if (!abrir || !form) return;
  const caixa = abrir.closest(".apoiar-mural");
  const secao = abrir.closest(".apoiar");
  const mostrar = (on) => {
    form.hidden = !on;
    caixa.classList.toggle("aberto", on);
    if (secao) secao.classList.toggle("mural-aberto", on);
    abrir.setAttribute("aria-expanded", String(on));
    if (on) { const n = form.querySelector("input[name=nome]"); if (n) n.focus({ preventScroll: true }); } else abrir.focus({ preventScroll: true });
  };
  abrir.addEventListener("click", () => mostrar(true));
  if (fechar) fechar.addEventListener("click", () => mostrar(false));
})();

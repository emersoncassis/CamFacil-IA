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
  for (const [k, v] of Object.entries(attrs)) { if (k === "class") e.className = v; else e.setAttribute(k, v); }
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

// ---------- Navegação ----------
function mostrar(id) {
  if (typeof vozParar === "function") vozParar();
  if (typeof fecharLeitor === "function" && leitorStream) fecharLeitor();
  document.querySelectorAll("[role=tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === id)));
  document.querySelectorAll("main [role=tabpanel]").forEach((p) => { p.hidden = p.id !== id; });
  if (id === "agentes") carregarAgentes();
  if (id === "dispositivos") carregarDispositivos();
}

// ---------- Agentes ----------
async function carregarAgentes() {
  const caixa = $("#lista-agentes");
  try {
    const { agentes } = await api("/api/agentes");
    if (!agentes.length) {
      caixa.replaceChildren(el("div", { class: "block cliente-vazio" },
        el("h2", {}, "Nenhum cliente ainda"),
        el("p", { class: "hint" }, "Instale o CamFácil IA no computador do cliente e pareie com este painel. As câmeras dele aparecem aqui."),
        el("div", { class: "actions" },
          el("button", { class: "btn primary", type: "button", onclick: () => mostrar("comecar") }, "Instalar num cliente"),
          el("button", { class: "btn", type: "button", onclick: () => $("#como-parear").click() }, "Já instalei, parear"))));
      return;
    }
    caixa.replaceChildren(...agentes.map((a) => {
      const campo = el("input", { class: "cell-edit cliente-nome", value: a.apelido_cliente, placeholder: "Nome do cliente", "aria-label": "Nome do cliente", maxlength: "60" });
      campo.addEventListener("change", async () => {
        try { await api(`/api/agentes/${a.id}`, "PATCH", { apelido_cliente: campo.value }); } catch (e) { campo.value = a.apelido_cliente; alert(e.message); }
      });
      return el("article", { class: "cliente", "data-agente": a.id },
        el("header", {}, campo,
          el("span", { class: "state", "data-online": a.id }, el("span", { class: "dot " + (a.online ? "on" : "") }), a.online ? "On-line" : "Desconectado"),
          el("span", { class: "tiny mono", title: `Impressão ${a.impressao} · pareado em ${data(a.pareado_em)}` }, (a.estado && a.estado.versao) || a.versao || "")),
        el("div", { "data-estado": a.id }, visaoAgente(a.estado, a.estado_em, a.online)));
    }));
  } catch (e) {
    caixa.replaceChildren(el("p", { class: "msg err" }, e.message));
  }
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

function visaoAgente(e, em, online) {
  if (!e) return el("p", { class: "tiny vazio-agente" }, online
    ? "Conectado. O primeiro resumo chega em alguns segundos (atualize o programa no cliente se não aparecer)."
    : "Ainda sem resumo deste computador. Ele aparece quando o programa estiver aberto e com internet.");
  const cams = e.cameras || [];
  const on = cams.filter((c) => c.online).length;
  // Ao vivo pelo Tailscale: abre a página leve do próprio programa do cliente.
  const ts = online && e.tailscale && e.tailscale.url;
  const aoVivo = (q, rotulo, cls) => el("a", { class: cls, href: `${ts}/ao-vivo${q}`, target: "_blank", rel: "noopener" }, rotulo);
  const camLista = el("ul", { class: "cams-agente" }, ...(cams.length ? cams.map((c) =>
    el("li", {}, el("span", { class: "dot " + (c.online ? "on" : "off") }),
      ts && c.id != null ? aoVivo(`?camera=${c.id}`, c.nome || "Câmera", "cam-link") : el("span", {}, c.nome || "Câmera"),
      el("span", { class: "tiny" }, c.online ? `${c.fps ?? 0} q/s${c.regras ? ` · ${c.regras} ${c.regras === 1 ? "regra" : "regras"}` : ""}${c.saude ? ` · ${c.saude}` : ""}` : (c.erro || "sem sinal"))))
    : [el("li", { class: "tiny" }, "Nenhuma câmera ligada ainda.")]));
  const ramPct = e.ram_total_mb ? (e.ram_usada_mb * 100) / e.ram_total_mb : 0;
  const maquina = el("div", { class: "maq-agente" },
    barraUso("CPU", e.cpu, `${Math.round(e.cpu || 0)}%`),
    barraUso("Memória", ramPct, e.ram_total_mb ? `${(e.ram_usada_mb / 1024).toFixed(1)} de ${(e.ram_total_mb / 1024).toFixed(1)} GB` : "—"),
    el("p", { class: "tiny" }, [e.maquina, e.sistema, e.proprio_mb != null ? `CamFácil IA usando ${e.proprio_mb} MB` : null].filter(Boolean).join(" · ")));
  const alertas = (e.alertas || []).slice(0, 5);
  const alertLista = el("ul", { class: "alertas-agente" }, ...(alertas.length ? alertas.map((a) =>
    el("li", {}, el("span", {}, `${a.camera || "Câmera"}${a.regra ? ` · ${a.regra}` : ""}`),
      el("span", { class: "tiny" }, `${TIPO_ALERTA[a.tipo] || a.tipo} · ${quandoRel(a.quando)}`)))
    : [el("li", { class: "tiny" }, "Nenhum alerta ainda.")]));
  const atual = em ? `Atualizado ${quandoRel(Date.parse(em) / 1000)}` : "";
  const barraVivo = el("div", { class: "vivo-barra" }, ts
    ? aoVivo("", "Ver câmeras ao vivo", "btn primary")
    : el("p", { class: "tiny" }, online
      ? "Para ver ao vivo, ligue o Tailscale no computador do cliente e no seu celular (veja como, abaixo)."
      : "Ao vivo fica disponível quando o computador do cliente estiver ligado."));
  return el("div", { class: "agente-visao" },
    barraVivo,
    el("section", {}, el("h3", {}, `Câmeras · ${on} de ${cams.length} no ar`), camLista),
    el("section", {}, el("h3", {}, "Máquina do cliente"), maquina),
    el("section", {}, el("h3", {}, "Últimos alertas"), alertLista),
    el("p", { class: "tiny atualizado" }, [atual, (e.modulos || []).length ? `IA ligada: ${e.modulos.join(", ")}` : "Nenhuma IA ligada"].filter(Boolean).join(" · ")));
}

// ---------- Dispositivos ----------
async function carregarDispositivos() {
  const corpo = $("#lista-dispositivos");
  const atual = await idbGet(`dispositivo:${eu.id}`);
  const { dispositivos } = await api("/api/dispositivos");
  const ativos = dispositivos.filter((d) => !d.revogado);
  if (!ativos.length) { corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "4" }, "Nenhum dispositivo ativo."))); return; }
  corpo.replaceChildren(...ativos.map((d) => {
    const btn = el("button", { class: "btn sm", type: "button" }, "Revogar");
    btn.onclick = async () => {
      if (!confirm(`Revogar "${d.apelido || "dispositivo"}"? Os agentes deixam de aceitar comandos dele.`)) return;
      await api(`/api/dispositivos/${d.id}`, "DELETE");
      carregarDispositivos();
    };
    const nome = d.apelido || "Dispositivo";
    return el("tr", {},
      el("td", {}, atual && atual.id === d.id ? `${nome} (este)` : nome),
      el("td", { class: "mono" }, d.impressao),
      el("td", {}, data(d.criado_em)),
      el("td", { class: "right" }, btn));
  }));
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
  $("#leitor").hidden = true;
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
      setTimeout(() => { history.replaceState(null, "", "/"); mostrar("agentes"); }, 1200);
    } else if (m.tipo === "pareamento_recusado") {
      msgPar("O pareamento foi recusado na tela do cliente.", "err");
    } else if (m.tipo === "estado") {
      const td = document.querySelector(`[data-estado="${m.agente_id}"]`);
      if (td) td.replaceChildren(visaoAgente(m.estado, new Date().toISOString(), true));
    } else if (["agente_online", "agente_offline", "despareado"].includes(m.tipo) && !$("#agentes").hidden) {
      carregarAgentes();
    }
  };
}

// ---------- Download do programa (Agente-Cam-Elo-IA embutido) ----------
const REL = "https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/";
const SISTEMAS = {
  windows: { nome: "Windows 10 e 11", ico: "i-win", arquivo: "camfacil-windows-x86_64.exe", cmd: "irm https://cfia.ysafe.com.br/instalar.ps1 | iex", onde: "PowerShell" },
  linux: { nome: "Linux (PC ou mini PC)", ico: "i-tux", arquivo: "camfacil-linux-x86_64", cmd: "curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh", onde: "terminal" },
  rpi: { nome: "Raspberry Pi e TV Box (ARM 64)", ico: "i-rpi", arquivo: "camfacil-linux-arm64", cmd: "curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh", onde: "terminal" },
  android: { nome: "Android (celular e TV Box)", ico: "i-phone", arquivo: "camfacil-android.apk" },
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

function montarDownload(alvo) {
  let atual = sistemaDoVisitante();
  const corpo = el("div", { class: "dl-corpo" });
  const abas = el("div", { class: "seg", role: "radiogroup", "aria-label": "Sistema" });
  const desenhar = () => {
    const s = SISTEMAS[atual];
    if (atual === "android") {
      corpo.replaceChildren(
        el("div", { class: "actions" },
          el("a", { class: "btn primary grande", href: REL + s.arquivo, download: "" }, icone("i-down"), el("span", {}, "Baixar o app Android")),
          el("span", { class: "tiny" }, "Grátis · cerca de 8 MB · Android 7 ou mais novo")),
        el("ol", { class: "steps mt" },
          el("li", {}, "Baixe pelo celular ou pela TV Box e toque no arquivo."),
          el("li", {}, "Se o Android avisar, toque em ", el("strong", {}, "Configurações"), " e ative ", el("strong", {}, "Permitir desta fonte"), ". O app é gratuito e fora da Play Store."),
          el("li", {}, "Escolha ", el("strong", {}, "Este aparelho é o servidor"), " para ligar as câmeras nele, ou ", el("strong", {}, "Ver um servidor à distância"), " para ver de longe pelo Tailscale.")));
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
        el("a", { class: "btn primary grande", href: REL + s.arquivo, download: "" }, icone("i-down"), el("span", {}, atual === "windows" ? "Baixar para Windows" : `Baixar para ${s.nome}`)),
        el("span", { class: "tiny" }, "Grátis · cerca de 4 MB · sem cadastro")),
      el("ol", { class: "steps mt" }, ...passos),
      el("p", { class: "tiny mt" }, atual === "windows" ? "Prefere instalar com atalho na área de trabalho? Cole no PowerShell:" : "Comando de instalação:"),
      caixaComando(s.cmd),
      el("p", { class: "tiny mt" }, "Para mostrar o vídeo, o programa usa o ffmpeg (gratuito). Se faltar, ele mesmo instala ou mostra como."));
    ligarCopiar(corpo);
  };
  // Um nome de grupo só por seletor, para os botões se alternarem.
  const grupo = "so-" + Math.random().toString(36).slice(2);
  for (const [id, s] of Object.entries(SISTEMAS)) {
    const input = el("input", { type: "radio", name: grupo, value: id });
    input.checked = id === atual;
    input.onchange = () => { atual = id; desenhar(); };
    abas.append(el("label", {}, input, el("span", {}, icone(s.ico, "ico sm"), s.nome)));
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
const voz = new Audio();
let vozFila = [];
let vozBotao = null;

function vozParar() {
  voz.pause(); vozFila = [];
  if (vozBotao) { vozBotao.querySelector("use").setAttribute("href", "#i-play"); vozBotao.classList.remove("tocando"); vozBotao = null; }
}

function vozProxima() {
  const f = vozFila.shift();
  if (!f) { vozParar(); return; }
  voz.src = `/audio/${f}.ogg`;
  voz.play().catch(vozParar);
}
voz.onended = vozProxima;

async function prepararNarracao() {
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
      vozProxima();
    };
  }
}

// ---------- Início ----------
async function main() {
  const erro = new URLSearchParams(location.search).get("erro");
  try {
    eu = await api("/api/eu");
  } catch (_) {
    if (location.pathname === "/parear") sessionStorage.setItem("cfia-voltar", location.pathname + location.hash);
    $("#entrar").hidden = false;
    if (erro) $("#entrar-erro").textContent = "Login cancelado. Tente de novo.";
    document.querySelectorAll("#entrar [data-baixar]").forEach(montarDownload);
    ligarCopiar($("#entrar"));
    prepararNarracao();
    return;
  }
  const voltar = sessionStorage.getItem("cfia-voltar");
  if (voltar) { sessionStorage.removeItem("cfia-voltar"); history.replaceState(null, "", voltar); }

  $("#app").hidden = false;
  $("#eu-email").textContent = eu.email;
  document.querySelectorAll("[role=tab]").forEach((b) => { b.onclick = () => { history.replaceState(null, "", "/"); mostrar(b.dataset.tab); }; });
  $("#como-parear").onclick = () => { $("#passos").hidden = !$("#passos").hidden; if ($("#passos").hidden) fecharLeitor(); };
  $("#ler-qr").onclick = abrirLeitor;
  $("#leitor-fechar").onclick = () => { fecharLeitor(); msgLeitor(""); };
  $("#digitar-codigo").onclick = () => { fecharLeitor(); msgLeitor(""); $("#form-codigo").hidden = false; $("#form-codigo").codigo.focus(); };
  $("#form-codigo").onsubmit = (ev) => { ev.preventDefault(); irParaParear(ev.target.codigo.value, null); };
  $("#sair").onclick = async () => { await api("/api/auth/sair", "POST"); location.href = "/"; };

  document.querySelectorAll("#app [data-baixar]").forEach(montarDownload);
  prepararNarracao();
  garantirDispositivo().catch(() => {});
  conectarPainel();
  if (location.pathname === "/parear") { telaParear(); return; }
  // Logado, vai direto ao painel. Sem cliente ainda, o próprio painel mostra o primeiro passo.
  mostrar("agentes");
}

main();

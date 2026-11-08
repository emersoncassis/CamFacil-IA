// Ao vivo: mosaico de 1 a 4 câmeras e, embaixo, o que está rodando: aparelho,
// IA, alertas, para quem vai e serviços ativos. Tela única, sem rolagem.
// Só a imagem e o nome da câmera: nada de caixas da IA (regra 4).
(() => {
  const $ = (s) => document.querySelector(s);
  let txt = {};
  const t = (k, v = {}) => (txt[k] || k).replace(/\{(\w+)\}/g, (_, n) => (v[n] ?? ""));
  const el = (tag, attrs = {}, ...filhos) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) if (v != null) e.setAttribute(k, v);
    for (const f of filhos.flat()) if (f != null) e.append(f instanceof Node ? f : document.createTextNode(String(f)));
    return e;
  };
  const get = async (u) => { try { const r = await fetch(u); return r.ok ? await r.json() : null; } catch (_) { return null; } };
  const guardar = (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* sem storage */ } };
  const ler = (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } };

  const q = new URLSearchParams(location.search);
  let n = Number(ler("cf-vivo-n")) || 4;
  let pagina = 0;
  let foco = q.get("camera");
  let nAntes = null;
  if (foco) { nAntes = n; n = 1; }
  let cams = [];
  const regras = {}; // camera_id -> [regras]

  // ---------- Stream da visualização (extra × principal) ----------
  // Só muda o que se vê: a IA continua analisando no stream extra.
  const streamDe = (id) => { try { return (JSON.parse(ler("cf-stream") || "{}")[id]) === "principal" ? "principal" : "extra"; } catch (_) { return "extra"; } };
  const guardarStream = (id, v) => { let m = {}; try { m = JSON.parse(ler("cf-stream") || "{}"); } catch (_) { m = {}; } if (v === "principal") m[id] = v; else delete m[id]; guardar("cf-stream", JSON.stringify(m)); };
  const principal = (c) => c.stream_principal === true && streamDe(c.id) === "principal";
  const qStream = (c) => (principal(c) ? "stream=principal&" : "");

  /// Aviso antes de ir para o principal (diálogo da página, não confirm()).
  function avisoPrincipal() {
    const d = $("#stream-dlg");
    return new Promise((ok) => {
      const fim = (v) => { d.close(); ok(v); };
      $("#stream-sim").onclick = () => fim(true);
      $("#stream-nao").onclick = () => fim(false);
      d.oncancel = (e) => { e.preventDefault(); fim(false); };
      d.showModal();
      $("#stream-nao").focus();
    });
  }

  function pilulaStream(c, aoTrocar) {
    if (c.stream_principal == null) return null; // demo/webcam/arquivo ou já no principal
    const travado = c.stream_principal === false;
    const p = principal(c);
    const atual = t(p ? "stream.principal" : "stream.extra");
    const b = el("button", { type: "button", class: "stream-pill", "aria-pressed": String(p), "aria-disabled": String(travado),
      title: travado ? t("stream.sem_padrao") : t("stream.ia_extra"),
      "aria-label": travado ? `${t("stream.rotulo")}: ${atual}. ${t("stream.sem_padrao")}` : t("stream.botao", { atual, outro: t(p ? "stream.extra" : "stream.principal") }) },
    el("span", { class: "stream-ponto", "aria-hidden": "true" }), atual);
    const dica = el("p", { class: "stream-dica", role: "status", hidden: "" }, t("stream.sem_padrao"));
    b.onclick = async (ev) => {
      ev.stopPropagation();
      if (travado) { dica.hidden = !dica.hidden; return; }
      if (!p && !(await avisoPrincipal())) return;
      guardarStream(c.id, p ? "extra" : "principal");
      aoTrocar();
    };
    dica.onclick = (ev) => ev.stopPropagation();
    return [b, dica];
  }

  // ---------- Balões das regras (iguais aos da tela Câmeras) ----------
  // Um balão por regra no centro da linha/zona, alinhado à imagem real
  // (object-fit: contain). Nunca caixas da IA (regra 4).
  const svgNS = "http://www.w3.org/2000/svg";
  const svgEl = (tag, attrs) => { const e = document.createElementNS(svgNS, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); return e; };
  const pts = (l) => l.map((p) => `${p.x * 1000},${p.y * 1000}`).join(" ");
  const centro = (r) => {
    const p = r.pontos || [];
    if (!p.length) return { x: 0.5, y: 0.5 };
    if (r.forma === "linha") return { x: (p[0].x + p[p.length - 1].x) / 2, y: (p[0].y + p[p.length - 1].y) / 2 };
    return { x: p.reduce((a, q) => a + q.x, 0) / p.length, y: p.reduce((a, q) => a + q.y, 0) / p.length };
  };
  const textoBalao = (r, e) => {
    const aviso = e && e.aviso ? ` · ${e.aviso}` : "";
    if (r.forma === "zona") return `${r.nome} · ${e && e.ocupada ? (r.rotulo_on || t("regra.ocupada")) : (r.rotulo_off || t("regra.livre"))}${aviso}`;
    return `${r.nome} · ${e ? e.contagem || 0 : 0}${aviso}`;
  };

  /// Põe a camada dos balões exatamente sobre a imagem (object-fit: contain).
  function encaixar(tile) {
    const img = tile.querySelector("img"); const camada = tile.querySelector("[data-camada]");
    if (!img || !camada) return;
    const nw = img.naturalWidth, nh = img.naturalHeight, W = tile.clientWidth, H = tile.clientHeight;
    if (!nw || !nh || !W || !H) { camada.hidden = true; return; }
    const k = Math.min(W / nw, H / nh); const w = nw * k, h = nh * k;
    Object.assign(camada.style, { left: `${(W - w) / 2}px`, top: `${(H - h) / 2}px`, width: `${w}px`, height: `${h}px` });
    camada.hidden = false;
  }

  function desenharBaloes(tile, id, e) {
    const camada = tile.querySelector("[data-camada]");
    if (!camada) return;
    const rs = regras[id] || [];
    const formas = ler("cf-formas") === "1";
    const svg = camada.querySelector("svg");
    const chaveFormas = formas ? rs.map((r) => r.id + (r.ajustes && r.ajustes.cor || "") + pts(r.pontos || [])).join("|") : "";
    if (svg.dataset.chave !== chaveFormas) {
      svg.dataset.chave = chaveFormas;
      svg.replaceChildren(...(formas ? rs.map((r) => {
        const cor = r.ajustes && r.ajustes.cor ? ` cor-${r.ajustes.cor}` : "";
        return r.forma === "linha"
          ? svgEl("polyline", { class: "vv-forma linha" + cor, points: pts(r.pontos || []) })
          : svgEl("polygon", { class: "vv-forma" + cor, points: pts(r.pontos || []) });
      }) : []));
    }
    const caixa = camada.querySelector("[data-baloes]");
    const vistos = new Set();
    for (const r of rs) {
      const est = e && e.regras ? e.regras[r.id] : null;
      let b = caixa.querySelector(`[data-regra="${r.id}"]`);
      if (!b) { b = el("div", { class: "cf-bubble", "data-regra": r.id }); caixa.append(b); }
      const c = centro(r);
      b.style.left = `${c.x * 100}%`; b.style.top = `${c.y * 100}%`;
      const txtNovo = textoBalao(r, est);
      if (b.textContent !== txtNovo) b.textContent = txtNovo;
      // Só troca a classe na mudança: a piscada não reinicia a cada atualização.
      b.classList.toggle("is-alert", !!(est && est.alerta_ms > 0));
      vistos.add(String(r.id));
    }
    caixa.querySelectorAll("[data-regra]").forEach((b) => { if (!vistos.has(b.dataset.regra)) b.remove(); });
  }

  const observador = typeof ResizeObserver === "function"
    ? new ResizeObserver(() => document.querySelectorAll(".vv-tile[data-id]").forEach(encaixar)) : null;

  // ---------- Som do alerta (como na tela Câmeras) ----------
  // Toca quando uma regra de QUALQUER câmera dispara (mesmo fora da página do
  // mosaico) ou a saúde passa de ok para problema. Respeita o "Som do alerta"
  // de cada regra e o da saúde (mapa "cf-som", padrão ligado).
  const SAUDE_PROBLEMA = ["escura", "coberta", "desfocada", "sem_sinal"];
  const somLigado = (id) => { try { return (JSON.parse(ler("cf-som") || "{}")[id]) !== false; } catch (_) { return true; } };
  let mudo = ler("cf-vivo-mudo") === "1";
  let somCtx = null;
  const alertaAntes = {};
  function liberarSom() {
    try {
      somCtx = somCtx || new (window.AudioContext || window.webkitAudioContext)();
      if (somCtx.state === "suspended") somCtx.resume();
      guardar("cf-vivo-som", "1");
    } catch (_) { /* sem áudio */ }
    mostrarSom();
  }
  function mostrarSom() {
    const rodando = somCtx && somCtx.state === "running";
    $("#som-ligar").hidden = mudo || !!rodando;
    const m = $("#som-mudo");
    m.setAttribute("aria-pressed", String(mudo));
    m.setAttribute("aria-label", t(mudo ? "vivo.som_desmutar" : "vivo.som_mutar"));
    m.title = m.getAttribute("aria-label");
  }
  function tocarAlerta(som) {
    if (mudo || !somCtx) return;
    CFSons.tocar(somCtx, som || "bipe");
  }
  /// Confere as mudanças de todas as câmeras e toca uma vez se algo disparou.
  function conferirAlertas() {
    let tocar = false;
    let somEscolhido = "";
    for (const c of cams) {
      const e = estadoCams[String(c.id)];
      if (!e) continue;
      const chaveS = `saude:${c.id}`;
      const problema = SAUDE_PROBLEMA.includes(e.saude);
      if (problema && alertaAntes[chaveS] === false && somLigado(chaveS)) tocar = true;
      alertaAntes[chaveS] = problema;
      for (const r of regras[c.id] || []) {
        const er = e.regras ? e.regras[r.id] : null;
        const agora = !!(er && er.alerta_ms > 0);
        const chave = `${c.id}:${r.id}`;
        if (agora && alertaAntes[chave] === false && somLigado(r.id)) { if (!tocar) somEscolhido = (r.ajustes && r.ajustes.som) || ""; tocar = true; }
        alertaAntes[chave] = agora;
      }
    }
    if (tocar) tocarAlerta(somEscolhido);
    return tocar;
  }

  // ---------- Mosaico ----------
  function visiveis() {
    if (n === 1 && foco) { const c = cams.find((x) => String(x.id) === String(foco)); if (c) return [c]; }
    return cams.slice(pagina * n, pagina * n + n);
  }

  function montar() {
    const m = $("#mosaico");
    m.dataset.n = String(n);
    document.querySelectorAll(".vv-layouts button").forEach((b) => b.setAttribute("aria-checked", String(Number(b.dataset.n) === n)));
    const paginas = Math.max(1, Math.ceil(cams.length / n));
    if (pagina >= paginas) pagina = paginas - 1;
    $("#pag").hidden = paginas < 2 || (n === 1 && foco);
    $("#pag-txt").textContent = `${pagina + 1}/${paginas}`;
    const lista = visiveis();
    const tiles = lista.map((c) => {
      const img = el("img", { alt: c.nome || t("vivo.camera") });
      // Uma câmera: vídeo contínuo (MJPEG). Mosaico: fotos em sequência, uma
      // de cada vez por quadro. O navegador só abre 6 conexões por endereço, e
      // 4 vídeos contínuos + o status travariam algum quadro.
      if (lista.length === 1) {
        const abrir = () => { img.src = `/api/cameras/${c.id}/mjpeg?${qStream(c)}t=${Date.now()}`; };
        img.onerror = () => setTimeout(() => { if (img.isConnected) abrir(); }, 5000);
        abrir();
      } else {
        const proxima = () => { if (img.isConnected || !img.dataset.vivo) { img.dataset.vivo = "1"; img.src = `/api/cameras/${c.id}/foto.jpg?${qStream(c)}t=${Date.now()}`; } };
        img.onload = () => setTimeout(() => { if (img.isConnected) proxima(); }, 120);
        img.onerror = () => setTimeout(() => { if (img.isConnected) proxima(); }, 3000);
        proxima();
      }
      img.addEventListener("load", () => { const tl = img.closest(".vv-tile"); if (tl) encaixar(tl); });
      const tile = el("div", { class: "vv-tile", "data-id": c.id, title: t("vivo.toque") },
        img,
        el("div", { class: "vv-camada", "data-camada": "", "aria-hidden": "true", hidden: "" },
          svgEl("svg", { viewBox: "0 0 1000 1000", preserveAspectRatio: "none" }),
          el("div", { class: "vv-baloes", "data-baloes": "" })),
        el("span", { class: "vv-rot" }, el("i", { class: "dot", "data-dot": "" }), c.nome || t("vivo.camera")),
        el("div", { class: "vv-info", "data-info": "" }),
        pilulaStream(c, montar));
      tile.onclick = () => {
        if (n === 1 && nAntes) { n = nAntes; nAntes = null; foco = null; } else { nAntes = n; n = 1; foco = String(c.id); }
        montar();
      };
      return tile;
    });
    while (tiles.length < n) {
      tiles.push(el("div", { class: "vv-tile vazio" },
        cams.length ? el("span", {}, t("vivo.vazio")) : el("span", {}, t("vivo.sem_cameras"), " ", el("a", { href: "/" }, t("vivo.adicionar")))));
    }
    // Para os vídeos antigos de verdade (libera a conexão MJPEG).
    m.querySelectorAll("img").forEach((i) => { i.src = ""; });
    m.replaceChildren(...tiles);
    if (observador) { observador.disconnect(); observador.observe(m); }
    atualizarTiles();
  }

  let estadoCams = {};
  function atualizarTiles() {
    document.querySelectorAll(".vv-tile[data-id]").forEach((tile) => {
      const id = tile.dataset.id; const e = estadoCams[id] || {};
      const dot = tile.querySelector("[data-dot]");
      dot.className = "dot " + (e.online ? (e.saude && e.saude !== "ok" ? "warn" : "on") : "off");
      const info = tile.querySelector("[data-info]");
      const rs = regras[id] || [];
      const estRegras = e.regras || {};
      const agora = Object.values(estRegras).some((r) => r && (r.ocupada || (r.alerta_ms && r.alerta_ms < 10000)));
      const itens = [];
      if (e.online) itens.push(el("span", {}, `${Math.round((e.fps || 0) * 10) / 10} q/s`));
      else itens.push(el("span", {}, e.erro ? t("vivo.sem_sinal") : t("vivo.ligando")));
      itens.push(el("span", {}, rs.length ? t(rs.length === 1 ? "vivo.regra" : "vivo.regras", { n: rs.length }) : t("vivo.sem_regra")));
      if (e.saude && e.saude !== "ok") itens.push(el("span", {}, t("saude." + e.saude)));
      if (agora) itens.push(el("span", { class: "alerta" }, t("vivo.alerta_agora")));
      info.replaceChildren(...itens);
      desenharBaloes(tile, id, e);
    });
  }

  // ---------- Status ----------
  const lin = (dot, rotulo, valor) => el("div", { class: "lin" }, dot ? el("i", { class: "dot " + dot }) : null, el("span", {}, rotulo), valor != null ? el("span", { class: "v" }, valor) : null);
  const barra = (rotulo, pct, valor) => el("div", { class: "lin" }, el("span", {}, rotulo),
    (() => { const b = el("span", { class: "barra" + (pct >= 85 ? " alto" : "") }); const i = el("i"); i.style.width = `${Math.max(0, Math.min(100, pct || 0))}%`; b.append(i); return b; })(),
    el("span", { class: "v" }, valor));
  const tempo = (s) => { s = Math.max(0, Math.round(s || 0)); if (s < 3600) return `${Math.round(s / 60)} min`; if (s < 86400) return `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min`; return `${Math.floor(s / 86400)} d ${Math.round((s % 86400) / 3600)} h`; };
  const hora = (q) => new Date(q * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  // Cabe quantas linhas no cartão? (nada de rolar: o resto fica de fora)
  const caber = (alvo, itens) => {
    alvo.replaceChildren(...itens);
    let i = itens.length;
    while (i > 1 && alvo.scrollHeight > alvo.clientHeight + 1) { itens[--i].remove(); }
  };

  async function status() {
    const [hw, sys, ia, mod, ev, elo, tg, wa, wh, arq, mcp, ts, rede] = await Promise.all([
      get("/api/hardware"), get("/api/system"), get("/api/ia"), get("/api/modulos"), get("/api/eventos?limite=12"),
      get("/api/elo"), get("/api/telegram"), get("/api/whatsapp"), get("/api/webhook"), get("/api/arquivo"), get("/api/mcp"), get("/api/tailscale"),
      get("/api/rede/resumo"),
    ]);
    // Última correção automática de IP nas 24 h (aviso informativo, não erro).
    const autoIp = ((rede && rede.auto_avisos) || [])[0];
    const a = (hw && hw.atual) || {}; const s = (sys && sys.sistema) || {};

    // Aparelho
    caber($("#c-disp"), [
      lin(null, [s.hostname, s.os].filter(Boolean).join(" · ") || t("vivo.aparelho")),
      barra("CPU", a.cpu ?? s.cpu_usage, `${Math.round(a.cpu ?? s.cpu_usage ?? 0)}%`),
      barra(t("vivo.memoria"), a.ram_pct, a.ram_total_mb ? `${(a.ram_mb / 1024).toFixed(1)}/${(a.ram_total_mb / 1024).toFixed(1)} GB` : "—"),
      lin(null, t("vivo.temperatura"), a.temperatura != null ? `${Math.round(a.temperatura)} °C` : "—"),
      lin(null, t("vivo.ligado_ha"), tempo(a.uptime_s)),
      lin(null, t("vivo.programa_usa"), s.self_ram_mb != null ? `${s.self_ram_mb} MB` : "—"),
    ]);

    // IA
    const on = mod ? ["movimento", "cerca", "saude", "camera", "pessoas", "contagem", "permanencia", "calor"].filter((k) => mod[k]) : [];
    const total = Object.values(regras).reduce((x, r) => x + r.length, 0);
    const comRegra = Object.values(regras).filter((r) => r.length).length;
    const det = ia && ia.detector || {};
    let pessoas;
    if (ia && !ia.suportado) pessoas = lin("off", t("vivo.ia.pessoas"), t("vivo.ia.sem_suporte"));
    else if (ia && !ia.pronto) pessoas = lin("off", t("vivo.ia.pessoas"), t("vivo.ia.nao_baixado"));
    else if (det.carregado) pessoas = lin("on", `${t("vivo.ia.pessoas")} · ${ia.modelo.nome.replace(".onnx", "")}`, `${det.ultimo_ms} ms`);
    else pessoas = lin("warn", t("vivo.ia.pessoas"), t("vivo.ia.parado"));
    caber($("#c-ia"), [
      lin(total ? "on" : "off", t("vivo.ia.regras_ativas"), t("vivo.ia.regras_valor", { n: total, c: comRegra })),
      pessoas,
      lin(on.includes("movimento") ? "on" : "off", t("vivo.ia.leve"), t(on.includes("movimento") || on.includes("cerca") ? "vivo.rodando" : "vivo.desligado")),
      el("div", { class: "chips" }, ...(on.length ? on.map((k) => el("span", {}, t("vivo.mod." + k))) : [el("span", {}, t("vivo.nenhum_modulo"))])),
      det.inferencias ? lin(null, t("vivo.ia.inferencias"), det.inferencias.toLocaleString("pt-BR")) : null,
    ].filter(Boolean));

    // Alertas: primeiro os das regras; movimento só se não houver outro.
    const evs = (ev && ev.eventos) || [];
    const fortes = evs.filter((e) => e.tipo !== "movimento");
    const mostrar = (fortes.length ? fortes : evs).slice(0, 8);
    const nomeCam = (id) => (cams.find((c) => c.id === id) || {}).nome || t("vivo.camera");
    const nomeRegra = (cid, rid) => ((regras[cid] || []).find((r) => r.id === rid) || {}).nome;
    caber($("#c-alertas"), mostrar.length ? mostrar.map((e) => lin(e.tipo === "movimento" ? null : "on",
      [nomeCam(e.camera_id), nomeRegra(e.camera_id, e.regra_id) || t("vivo.ev." + e.tipo)].join(" · "), hora(e.quando)))
      : [el("p", { class: "mudo" }, t("vivo.sem_alertas"))]);

    // Para quem
    const tgOk = tg && tg.situacao && tg.situacao.configurado;
    const waOk = wa && wa.situacao && wa.situacao.configurado;
    const whOk = wh && wh.situacao && wh.situacao.configurado;
    let whHost = ""; try { whHost = whOk ? new URL(wh.situacao.url).host : ""; } catch (_) { whHost = ""; }
    caber($("#c-quem"), [
      lin(elo && elo.pareado ? "on" : "off", t("vivo.quem.painel"), elo && elo.pareado ? (elo.dono_email || t("vivo.pareado")) : t("vivo.nao_pareado")),
      lin(tgOk ? "on" : "off", "Telegram", tgOk ? t("vivo.contatos", { n: (tg.chats || []).length }) : t("vivo.desligado")),
      lin(waOk ? "on" : "off", "WhatsApp", waOk ? t("vivo.contatos", { n: (wa.contatos || []).length }) : t("vivo.desligado")),
      lin(whOk ? "on" : "off", "Webhook", whOk ? whHost : t("vivo.desligado")),
      lin(arq && arq.ligado ? "on" : "off", t("vivo.quem.arquivo"), arq && arq.ligado ? t("vivo.fotos_ate", { n: arq.manter }) : t("vivo.desligado")),
    ]);

    // Serviços
    caber($("#c-serv"), [
      lin(cams.length ? "on" : "off", t("vivo.serv.video"), t("vivo.cameras_no_ar", { n: Object.values(estadoCams).filter((e) => e.online).length, t: cams.length })),
      autoIp ? el("div", { class: "lin", title: autoIp.resultado === "oscilou" ? t("autoip.aviso_oscilou", autoIp) : t("autoip.aviso_corrigido", autoIp) },
        el("i", { class: "dot " + (autoIp.resultado === "oscilou" ? "warn" : "on") }),
        el("span", {}, t(autoIp.resultado === "oscilou" ? "vivo.serv.ip_oscilou" : "vivo.serv.ip")),
        el("span", { class: "v" }, autoIp.resultado === "oscilou" ? autoIp.nome : `${autoIp.nome} · ${autoIp.para}`)) : null,
      lin(elo && elo.conectado ? "on" : (elo && elo.pareado ? "warn" : "off"), t("vivo.serv.elo"), elo && elo.conectado ? t("vivo.conectado") : (elo && elo.pareado ? t("vivo.reconectando") : t("vivo.desligado"))),
      lin(ts && ts.escutando ? "on" : "off", "Tailscale", ts && ts.escutando ? (ts.ip || t("vivo.ligado")) : t(ts && ts.instalado ? "vivo.desligado" : "vivo.nao_instalado")),
      lin(tgOk && tg.situacao.escutando ? "on" : (tgOk ? "warn" : "off"), t("vivo.serv.comandos"), tgOk ? t(tg.situacao.escutando ? "vivo.ouvindo" : "vivo.parado") : t("vivo.desligado")),
      lin(mcp && mcp.ligado ? "on" : "off", "MCP", mcp && mcp.ligado ? t(mcp.escrita ? "vivo.mcp_escrita" : "vivo.mcp_leitura") : t("vivo.desligado")),
    ].filter(Boolean));
  }

  async function estado() {
    const e = await get("/api/estado");
    estadoCams = (e && e.cameras) || {};
    conferirAlertas();
    atualizarTiles();
  }

  async function carregarCameras() {
    const r = await get("/api/cameras");
    const novas = ((r && r.cameras) || []).filter((c) => c.ativa !== false);
    const chave = (l) => l.map((c) => `${c.id}${c.nome}${c.stream_principal}`).join();
    const mudou = chave(novas) !== chave(cams);
    cams = novas;
    await Promise.all(cams.map(async (c) => { const x = await get(`/api/cameras/${c.id}/regras`); regras[c.id] = ((x && x.regras) || []).filter((r) => !(r.ajustes && r.ajustes.ativa === false)); }));
    if (mudou) montar();
  }

  // ---------- Abas do status (celular) ----------
  function montarAbas() {
    const abas = $("#abas");
    const cartoes = [...document.querySelectorAll(".vv-c")];
    let atual = ler("cf-vivo-aba") || "ia";
    const marcar = () => {
      cartoes.forEach((c) => c.classList.toggle("aberto", c.dataset.c === atual));
      abas.querySelectorAll("button").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.c === atual)));
    };
    abas.replaceChildren(...cartoes.map((c) => {
      const b = el("button", { type: "button", role: "tab", "data-c": c.dataset.c }, c.querySelector("h2").textContent);
      b.onclick = () => { atual = c.dataset.c; guardar("cf-vivo-aba", atual); marcar(); status(); };
      return b;
    }));
    marcar();
  }

  async function iniciar() {
    txt = (await get("/i18n/pt-BR.json")) || {};
    document.querySelectorAll("[data-i18n]").forEach((e) => { e.textContent = t(e.dataset.i18n); });
    document.querySelectorAll("[data-i18n-title]").forEach((e) => { e.title = t(e.dataset.i18nTitle); });
    document.title = `${t("vivo.titulo")} · CamFácil IA`;
    document.querySelectorAll(".vv-layouts button").forEach((b) => {
      b.title = t("vivo.layout_n", { n: b.dataset.n });
      b.onclick = () => { n = Number(b.dataset.n); foco = null; nAntes = null; pagina = 0; guardar("cf-vivo-n", String(n)); montar(); };
    });
    $("#pag-ant").onclick = () => { const p = Math.max(1, Math.ceil(cams.length / n)); pagina = (pagina - 1 + p) % p; montar(); };
    $("#pag-prox").onclick = () => { const p = Math.max(1, Math.ceil(cams.length / n)); pagina = (pagina + 1) % p; montar(); };
    montarAbas();
    // Áudio só depois de um toque/clique (regra dos navegadores).
    $("#som-ligar").onclick = liberarSom;
    $("#som-mudo").onclick = () => { mudo = !mudo; guardar("cf-vivo-mudo", mudo ? "1" : "0"); if (!mudo) liberarSom(); mostrarSom(); };
    ["pointerdown", "keydown"].forEach((ev) => addEventListener(ev, () => { if (!mudo && (!somCtx || somCtx.state !== "running")) liberarSom(); }, { capture: true }));
    if (ler("cf-vivo-som") === "1" && !mudo) liberarSom(); // pode já valer se o navegador deixar
    mostrarSom();
    await carregarCameras();
    montar();
    estado(); status();
    setInterval(estado, 1500);
    setInterval(status, 5000);
    setInterval(carregarCameras, 30000);
    addEventListener("resize", () => { status(); document.querySelectorAll(".vv-tile[data-id]").forEach(encaixar); });
  }
  iniciar();
})();

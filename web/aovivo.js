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
        const abrir = () => { img.src = `/api/cameras/${c.id}/mjpeg?t=${Date.now()}`; };
        img.onerror = () => setTimeout(() => { if (img.isConnected) abrir(); }, 5000);
        abrir();
      } else {
        const proxima = () => { if (img.isConnected || !img.dataset.vivo) { img.dataset.vivo = "1"; img.src = `/api/cameras/${c.id}/foto.jpg?t=${Date.now()}`; } };
        img.onload = () => setTimeout(() => { if (img.isConnected) proxima(); }, 120);
        img.onerror = () => setTimeout(() => { if (img.isConnected) proxima(); }, 3000);
        proxima();
      }
      const tile = el("div", { class: "vv-tile", "data-id": c.id, title: t("vivo.toque") },
        img,
        el("span", { class: "vv-rot" }, el("i", { class: "dot", "data-dot": "" }), c.nome || t("vivo.camera")),
        el("div", { class: "vv-info", "data-info": "" }));
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
    const [hw, sys, ia, mod, ev, elo, tg, wa, wh, arq, mcp, ts] = await Promise.all([
      get("/api/hardware"), get("/api/system"), get("/api/ia"), get("/api/modulos"), get("/api/eventos?limite=12"),
      get("/api/elo"), get("/api/telegram"), get("/api/whatsapp"), get("/api/webhook"), get("/api/arquivo"), get("/api/mcp"), get("/api/tailscale"),
    ]);
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
    const on = mod ? ["movimento", "cerca", "saude", "pessoas", "contagem", "permanencia", "calor"].filter((k) => mod[k]) : [];
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
      lin(elo && elo.conectado ? "on" : (elo && elo.pareado ? "warn" : "off"), t("vivo.serv.elo"), elo && elo.conectado ? t("vivo.conectado") : (elo && elo.pareado ? t("vivo.reconectando") : t("vivo.desligado"))),
      lin(ts && ts.escutando ? "on" : "off", "Tailscale", ts && ts.escutando ? (ts.ip || t("vivo.ligado")) : t(ts && ts.instalado ? "vivo.desligado" : "vivo.nao_instalado")),
      lin(tgOk && tg.situacao.escutando ? "on" : (tgOk ? "warn" : "off"), t("vivo.serv.comandos"), tgOk ? t(tg.situacao.escutando ? "vivo.ouvindo" : "vivo.parado") : t("vivo.desligado")),
      lin(mcp && mcp.ligado ? "on" : "off", "MCP", mcp && mcp.ligado ? t(mcp.escrita ? "vivo.mcp_escrita" : "vivo.mcp_leitura") : t("vivo.desligado")),
    ]);
  }

  async function estado() {
    const e = await get("/api/estado");
    estadoCams = (e && e.cameras) || {};
    atualizarTiles();
  }

  async function carregarCameras() {
    const r = await get("/api/cameras");
    const novas = ((r && r.cameras) || []).filter((c) => c.ativa !== false);
    const mudou = novas.map((c) => c.id + c.nome).join() !== cams.map((c) => c.id + c.nome).join();
    cams = novas;
    await Promise.all(cams.map(async (c) => { const x = await get(`/api/cameras/${c.id}/regras`); regras[c.id] = (x && x.regras) || []; }));
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
    await carregarCameras();
    montar();
    estado(); status();
    setInterval(estado, 2000);
    setInterval(status, 5000);
    setInterval(carregarCameras, 30000);
    addEventListener("resize", () => status());
  }
  iniciar();
})();

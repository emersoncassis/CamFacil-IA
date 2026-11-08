// CamFácil IA — modo estúdio, versão básica (SDD-07). Assistente em seis
// passos numa tela só: descrever (receita conferida pelo núcleo), coletar,
// rotular (100% no navegador: as fotos nunca vão ao servidor), treinar no
// Colab, importar no programa e compartilhar a receita. JS puro, sem nada
// inline (CSP).
"use strict";
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const el = (tag, attrs = {}, ...kids) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (k === "class") e.className = v;
      else if (k.startsWith("on") && typeof v === "function") e[k] = v;
      else e.setAttribute(k, v);
    }
    e.append(...kids.filter((k) => k != null));
    return e;
  };
  const NS = "http://www.w3.org/2000/svg";
  const ico = (d) => { const s = document.createElementNS(NS, "svg"); s.setAttribute("class", "ico sm"); s.setAttribute("viewBox", "0 0 24 24"); const p = document.createElementNS(NS, "path"); p.setAttribute("d", d); s.append(p); return s; };

  async function api(caminho, metodo = "GET", corpo) {
    const opt = { method: metodo, headers: {}, credentials: "same-origin" };
    if (corpo !== undefined) { opt.headers["Content-Type"] = "application/json"; opt.body = JSON.stringify(corpo); }
    const r = await fetch(caminho, opt);
    const dados = await r.json().catch(() => ({}));
    if (!r.ok) { const e = new Error(dados.erro || r.statusText); e.status = r.status; e.erros = dados.erros; throw e; }
    return dados;
  }

  const guardar = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* sem armazenamento */ } };
  const ler = (k) => { try { return JSON.parse(localStorage.getItem(k) || "null"); } catch (_) { return null; } };

  function baixar(blob, nome) {
    const a = el("a", { href: URL.createObjectURL(blob), download: nome });
    document.body.append(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 4000);
  }

  // Texto fixo exigido pelo formato (ADR 0013); o núcleo confere.
  const AVISO = "Somente para estudo e aprendizagem. Não use em produção: para uso real, procure soluções homologadas (câmeras e gravadores com IA de borda ou plataformas corporativas de IA).";
  const sem = (s) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const slug = (s, sep, max) => sem(s).replace(/[^a-z0-9]+/g, sep).replace(new RegExp(`^\\${sep}+|\\${sep}+$`, "g"), "").slice(0, max).replace(new RegExp(`\\${sep}+$`), "");

  // ---------------------------------------------------------------- estado
  const PASSOS = ["descrever", "coletar", "rotular", "treinar", "importar", "compartilhar"];
  const S = {
    passo: 0,
    classes: [],        // [{id, nome}] na ordem dos índices do modelo
    idFixo: null,       // id de receita aberta de arquivo ou da galeria
    valida: null,       // receita canônica conferida pelo servidor
    fotos: [],          // [{nome, arquivo, caixas: [{c, x0, y0, x1, y1}], vazia}]
    atual: 0,
    classeAtiva: 0,
    sel: -1,
  };
  const form = $("#form-receita");

  // ---------------------------------------------------------------- navegação
  const botoesPasso = $$(".est-passos [data-ir]");
  function ir(n, focar) {
    S.passo = Math.max(0, Math.min(PASSOS.length - 1, n));
    const id = PASSOS[S.passo];
    PASSOS.forEach((p) => { $("#" + p).hidden = p !== id; });
    $("#vms").hidden = true; $(".est-nav").hidden = false; $("#abrir-vms").removeAttribute("aria-current");
    botoesPasso.forEach((b, i) => {
      b.setAttribute("aria-selected", String(i === S.passo));
      b.tabIndex = i === S.passo ? 0 : -1;
      b.classList.toggle("feito", i < S.passo);
    });
    $("#pos").textContent = `${S.passo + 1} de ${PASSOS.length}`;
    $("#ant").disabled = S.passo === 0;
    $("#prox").disabled = S.passo === PASSOS.length - 1;
    history.replaceState(null, "", S.passo ? `#${id}` : location.pathname);
    guardarRascunho();
    if (id === "rotular") { montarClassesRotulo(); requestAnimationFrame(desenhar); }
    if (id === "treinar") atualizarTreinar();
    if (id === "compartilhar") abrirCompartilhar();
    if (focar) $("#" + id + " h1").focus({ preventScroll: true });
  }
  botoesPasso.forEach((b, i) => {
    b.onclick = () => ir(i, true);
    b.onkeydown = (ev) => {
      const d = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[ev.key];
      if (d) { ev.preventDefault(); ir(S.passo + d); botoesPasso[S.passo].focus(); }
    };
  });
  $$(".est-passo h1").forEach((h) => { h.tabIndex = -1; });
  $("#ant").onclick = () => ir(S.passo - 1, true);
  $("#prox").onclick = async () => {
    if (PASSOS[S.passo] === "descrever" && !(await conferir())) return;
    ir(S.passo + 1, true);
  };

  // ---------------------------------------------------------------- 1. descrever
  function idReceita() {
    if (S.idFixo) return S.idFixo;
    let id = slug(form.nome.value, "-", 60);
    if (id.length < 3) id = (id ? id + "-" : "") + "estudo";
    return id;
  }
  function idClasse(nome, usados) {
    let base = slug(nome, "_", 36) || "objeto";
    if (!/^[a-z0-9]/.test(base)) base = "o" + base;
    let id = base; let n = 2;
    while (usados.has(id)) id = `${base}_${n++}`;
    return id;
  }

  function desenharChips() {
    $("#chips").replaceChildren(...S.classes.map((c, i) => el("span", { class: "chip" }, c.nome,
      el("button", { type: "button", "aria-label": `Tirar ${c.nome}`, onclick: () => tirarClasse(i) }, ico("M6 6l12 12M18 6 6 18")))));
  }
  function adicionarClasse() {
    const campo = form.nova_classe;
    const nome = campo.value.trim().replace(/\s+/g, " ");
    if (!nome) { campo.focus(); return; }
    if (S.classes.length >= 20) { resultado([], "No máximo 20 objetos: um modelo nano aprende melhor poucas classes."); return; }
    if (S.classes.some((c) => sem(c.nome) === sem(nome))) { campo.select(); return; }
    S.classes.push({ id: idClasse(nome, new Set(S.classes.map((c) => c.id))), nome: nome.slice(0, 40) });
    campo.value = "";
    mudou();
    desenharChips();
    campo.focus();
  }
  function tirarClasse(i) {
    S.classes.splice(i, 1);
    // As caixas já desenhadas acompanham a nova numeração.
    for (const f of S.fotos) f.caixas = f.caixas.filter((b) => b.c !== i).map((b) => (b.c > i ? { ...b, c: b.c - 1 } : b));
    S.classeAtiva = Math.min(S.classeAtiva, Math.max(0, S.classes.length - 1));
    mudou();
    desenharChips();
  }
  $("#add-classe").onclick = adicionarClasse;
  form.nova_classe.onkeydown = (ev) => { if (ev.key === "Enter") { ev.preventDefault(); adicionarClasse(); } };

  function regraAtual() { return form.regra.value; }
  function mostrarRegra() {
    const r = regraAtual();
    $$(".regra-extra [data-regra]").forEach((x) => { x.hidden = !x.dataset.regra.split(" ").includes(r); });
  }
  $$("input[name=regra]", form).forEach((x) => { x.onchange = () => { mostrarRegra(); mudou(); }; });
  ["nome", "local", "sentido", "janela", "permanencia"].forEach((n) => { form[n].addEventListener("input", mudou); });

  function mudou() {
    S.valida = null;
    $("#resultado").replaceChildren();
    atualizarReceitaAtual();
    guardarRascunho();
  }

  function fotosRotuladas() { return S.fotos.filter((f) => f.caixas.length || f.vazia).length; }

  function montarReceita() {
    const ids = S.classes.map((c) => c.id);
    const r = regraAtual();
    const linhas = []; const zonas = []; const contagens = [];
    if (r === "linha" || r === "contagem") {
      linhas.push({ id: "linha", nome: "Linha de passagem", objetos: ids, pontos: [{ x: 0.5, y: 0.1 }, { x: 0.5, y: 0.9 }], sentido: form.sentido.value });
    }
    if (r === "contagem") {
      contagens.push({ id: "contagem", nome: "Contagem na linha", objetos: ids, linha: "linha", zona: null, janela: form.janela.value });
    }
    if (r === "zona") {
      const p = Number(form.permanencia.value);
      zonas.push({ id: "zona", nome: "Área observada", objetos: ids, pontos: [{ x: 0.2, y: 0.2 }, { x: 0.8, y: 0.2 }, { x: 0.8, y: 0.8 }, { x: 0.2, y: 0.8 }], permanencia_s: p > 0 ? Math.round(p) : null });
    }
    return {
      formato: "camfacil-receita", versao: 1, id: idReceita(),
      nome: form.nome.value.trim(), descricao: "", somente_estudo: true, aviso: AVISO,
      modelo: { base: "yolo11n", lado: 640, classes: ids, confianca: 0.35 },
      objetos: S.classes.map((c) => ({ id: c.id, nome: c.nome, descricao: "" })),
      zonas, linhas, trajetorias: [], contagens,
      estudo: { autor: "", local: form.local.value.trim(), fotos: fotosRotuladas(), epocas: 0, metricas: null, notas: "" },
    };
  }

  function resultado(erros, titulo) {
    const caixa = $("#resultado");
    if (!erros.length && !titulo) { caixa.replaceChildren(); return; }
    const MAX = innerHeight < 700 ? 1 : innerHeight < 900 ? 2 : 4;
    const lista = el("ul", { class: "res-erros" }, ...erros.slice(0, MAX).map((e) => el("li", {}, e)));
    if (erros.length > MAX) lista.append(el("li", {}, `e mais ${erros.length - MAX}; corrija ${MAX === 1 ? "esta" : "estas"} primeiro.`));
    caixa.replaceChildren(el("p", { class: "res-erro" }, titulo || "A receita ainda não passou:"), erros.length ? lista : null);
  }

  async function conferir() {
    const erros = [];
    if (form.nome.value.trim().length < 3) erros.push("Dê um nome com pelo menos 3 letras.");
    if (!S.classes.length) erros.push("Adicione pelo menos um objeto para detectar.");
    if (erros.length) { resultado(erros); return false; }
    $("#validar").disabled = true;
    try {
      const r = await api("/api/estudio/validar", "POST", { receita: montarReceita() });
      S.valida = r.receita;
      $("#resultado").replaceChildren(el("p", { class: "res-ok" }, "Receita conferida. ",
        el("span", { class: "mono" }, `${r.receita.id} · sha256 ${r.sha256.slice(0, 12)}…`)));
      atualizarReceitaAtual();
      guardarRascunho();
      return true;
    } catch (e) {
      resultado(e.erros && e.erros.length ? e.erros : [e.message]);
      return false;
    } finally { $("#validar").disabled = false; }
  }
  form.onsubmit = (ev) => { ev.preventDefault(); conferir(); };

  function preencher(r) {
    form.nome.value = r.nome || "";
    form.local.value = (r.estudo && r.estudo.local) || "";
    const nomes = Object.fromEntries((r.objetos || []).map((o) => [o.id, o.nome]));
    const ordem = (r.modelo && r.modelo.classes) || (r.objetos || []).map((o) => o.id);
    S.classes = ordem.map((id) => ({ id, nome: nomes[id] || id }));
    S.idFixo = r.id || null;
    const regra = r.contagens && r.contagens.length ? "contagem" : r.linhas && r.linhas.length ? "linha" : r.zonas && r.zonas.length ? "zona" : "nenhuma";
    $$("input[name=regra]", form).forEach((x) => { x.checked = x.value === regra; });
    if (r.linhas && r.linhas[0]) form.sentido.value = r.linhas[0].sentido || "ambos";
    if (r.contagens && r.contagens[0]) form.janela.value = r.contagens[0].janela || "dia";
    form.permanencia.value = (r.zonas && r.zonas[0] && r.zonas[0].permanencia_s) || "";
    mostrarRegra();
    desenharChips();
    mudou();
  }

  $("#abrir-receita").onchange = async (ev) => {
    const f = ev.target.files[0];
    ev.target.value = "";
    if (!f) return;
    try { preencher(JSON.parse(await f.text())); $(".est-mais").open = false; conferir(); }
    catch (_) { resultado([], `${f.name} não é uma receita JSON válida.`); }
  };
  $("#exemplo").onclick = () => {
    preencher({ nome: "Contar caixas de banana", id: "contar-caixas-banana", estudo: { local: "estoque de hortifrúti" }, objetos: [{ id: "caixa_banana", nome: "caixa de banana" }], linhas: [{ sentido: "entrada" }], contagens: [{ janela: "dia" }] });
    $(".est-mais").open = false;
  };
  document.addEventListener("click", (ev) => { const d = $(".est-mais"); if (d.open && !ev.target.closest(".est-mais")) d.open = false; });

  function atualizarReceitaAtual() {
    const p = $("#receita-atual");
    const nome = form.nome.value.trim();
    $$("[data-id-receita]").forEach((x) => { x.textContent = nome ? idReceita() : "sua-receita"; });
    if (!nome) { p.hidden = true; return; }
    p.hidden = false;
    const n = fotosRotuladas();
    p.replaceChildren("Receita: ", el("strong", {}, nome), el("br"),
      el("span", { class: "mono" }, idReceita()), ` · ${S.classes.length} objeto${S.classes.length === 1 ? "" : "s"}`,
      n ? ` · ${n} foto${n === 1 ? "" : "s"} rotulada${n === 1 ? "" : "s"}` : "",
      S.valida ? " · conferida" : "");
  }

  function guardarRascunho() {
    guardar("cfia-estudio", {
      passo: S.passo, nome: form.nome.value, local: form.local.value, classes: S.classes, idFixo: S.idFixo,
      regra: regraAtual(), sentido: form.sentido.value, janela: form.janela.value, permanencia: form.permanencia.value,
    });
  }
  function lerRascunho() {
    const r = ler("cfia-estudio");
    if (!r) return 0;
    form.nome.value = r.nome || ""; form.local.value = r.local || "";
    S.classes = Array.isArray(r.classes) ? r.classes.filter((c) => c && c.id && c.nome) : [];
    S.idFixo = r.idFixo || null;
    $$("input[name=regra]", form).forEach((x) => { x.checked = x.value === (r.regra || "nenhuma"); });
    if (r.sentido) form.sentido.value = r.sentido;
    if (r.janela) form.janela.value = r.janela;
    form.permanencia.value = r.permanencia || "";
    return Number(r.passo) || 0;
  }

  // ---------------------------------------------------------------- 3. rotular
  const tela = $("#tela");
  const ctx = tela.getContext("2d");
  const cache = new Map(); // índice → ImageBitmap (só a foto atual e vizinhas)
  const EXT = /\.(jpe?g|png|webp|bmp)$/i;

  function montarClassesRotulo() {
    const barra = $("#rot-classes-barra");
    if (!S.classes.length) {
      barra.replaceChildren(el("span", { class: "tiny" }, "Adicione os objetos no passo 1 para rotular."));
      return;
    }
    barra.replaceChildren(...S.classes.map((c, i) => el("button", {
      type: "button", role: "radio", "aria-checked": String(i === S.classeAtiva), title: `Tecla ${i + 1}`,
      onclick: () => escolherClasse(i),
    }, el("span", { class: "idx" }, String(i + 1)), c.nome)));
  }
  function escolherClasse(i) {
    if (i < 0 || i >= S.classes.length) return;
    S.classeAtiva = i;
    const f = S.fotos[S.atual];
    if (f && S.sel >= 0) { f.caixas[S.sel].c = i; desenhar(); }
    montarClassesRotulo();
  }

  async function carregarFotos(arquivos) {
    const lista = [...arquivos].filter((f) => EXT.test(f.name) || /^image\//.test(f.type)).sort((a, b) => a.name.localeCompare(b.name, "pt-BR", { numeric: true }));
    if (!lista.length) { $("#foto-info").textContent = "Nenhuma foto nessa escolha (jpg, png, webp ou bmp)."; return; }
    cache.forEach((b) => b.close && b.close());
    cache.clear();
    S.fotos = lista.map((arquivo) => ({ nome: arquivo.name, arquivo, caixas: [], vazia: false }));
    S.atual = 0; S.sel = -1;
    await mostrarFoto(0);
    tela.focus({ preventScroll: true });
  }
  $("#fotos").onchange = (ev) => { carregarFotos(ev.target.files); ev.target.value = ""; };
  $("#fotos-pasta").onchange = (ev) => { carregarFotos(ev.target.files); ev.target.value = ""; };
  $("#abrir-pasta").onclick = async () => {
    if (!window.showDirectoryPicker) { $("#fotos-pasta").click(); return; }
    try {
      const dir = await window.showDirectoryPicker({ mode: "read" });
      const arqs = [];
      for await (const h of dir.values()) if (h.kind === "file" && EXT.test(h.name)) arqs.push(await h.getFile());
      carregarFotos(arqs);
    } catch (e) { if (e.name !== "AbortError") $("#foto-info").textContent = "Não deu para abrir a pasta. Use Escolher fotos."; }
  };

  async function bitmap(i) {
    if (cache.has(i)) return cache.get(i);
    const b = await createImageBitmap(S.fotos[i].arquivo);
    cache.set(i, b);
    for (const k of [...cache.keys()]) if (Math.abs(k - i) > 1) { const v = cache.get(k); if (v.close) v.close(); cache.delete(k); }
    return b;
  }
  async function mostrarFoto(i) {
    if (!S.fotos.length) return;
    S.atual = (i + S.fotos.length) % S.fotos.length;
    S.sel = -1;
    try { await bitmap(S.atual); } catch (_) { /* foto ilegível: mostra o aviso no desenho */ }
    desenhar();
    if (S.atual + 1 < S.fotos.length) bitmap(S.atual + 1).catch(() => {});
  }
  $("#foto-ant").onclick = () => mostrarFoto(S.atual - 1);
  $("#foto-prox").onclick = () => mostrarFoto(S.atual + 1);

  let enc = { x: 0, y: 0, w: 1, h: 1 };
  const cor = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
  function desenhar() {
    const caixa = $("#palco").getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const W = Math.max(1, Math.round(caixa.width)); const H = Math.max(1, Math.round(caixa.height));
    if (tela.width !== W * dpr || tela.height !== H * dpr) { tela.width = W * dpr; tela.height = H * dpr; }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const f = S.fotos[S.atual];
    $("#rot-vazio").hidden = !!f;
    atualizarRodape();
    if (!f) return;
    const b = cache.get(S.atual);
    if (!b) { ctx.fillStyle = cor("--text-3"); ctx.font = `13px ${cor("--font")}`; ctx.fillText("Não deu para abrir esta foto.", 16, 24); return; }
    const esc = Math.min(W / b.width, H / b.height);
    enc = { w: b.width * esc, h: b.height * esc, x: 0, y: 0 };
    enc.x = (W - enc.w) / 2; enc.y = (H - enc.h) / 2;
    ctx.drawImage(b, enc.x, enc.y, enc.w, enc.h);
    const caixas = arrasto && arrasto.nova ? [...f.caixas, arrasto.nova] : f.caixas;
    ctx.font = `500 12px ${cor("--font")}`;
    ctx.textBaseline = "top";
    caixas.forEach((c, i) => {
      const sel = i === S.sel || (arrasto && c === arrasto.nova);
      const x = enc.x + Math.min(c.x0, c.x1) * enc.w; const y = enc.y + Math.min(c.y0, c.y1) * enc.h;
      const w = Math.abs(c.x1 - c.x0) * enc.w; const h = Math.abs(c.y1 - c.y0) * enc.h;
      ctx.lineWidth = sel ? 2 : 1.5;
      ctx.strokeStyle = sel ? cor("--accent") : cor("--text");
      ctx.strokeRect(x, y, w, h);
      const nome = (S.classes[c.c] || { nome: "?" }).nome;
      const tw = ctx.measureText(nome).width + 10;
      const ty = y - 18 >= enc.y ? y - 18 : y;
      ctx.fillStyle = sel ? cor("--accent") : "rgba(0, 0, 0, 0.72)";
      ctx.fillRect(x, ty, tw, 18);
      ctx.fillStyle = sel ? cor("--c-black") : cor("--text");
      ctx.fillText(nome, x + 5, ty + 3);
    });
  }
  function atualizarRodape() {
    const n = S.fotos.length;
    const f = S.fotos[S.atual];
    $("#foto-pos").textContent = n ? `${S.atual + 1} de ${n}` : "0 de 0";
    const rot = fotosRotuladas();
    $("#foto-info").textContent = n ? `${f.caixas.length} caixa${f.caixas.length === 1 ? "" : "s"} · ${rot} rotulada${rot === 1 ? "" : "s"}` : "";
    $("#apagar-caixa").disabled = S.sel < 0;
    $("#sem-objeto").setAttribute("aria-pressed", String(!!(f && f.vazia)));
    $("#sem-objeto").disabled = !f;
    $("#exportar").disabled = !rot;
    $("#foto-ant").disabled = n < 2; $("#foto-prox").disabled = n < 2;
    atualizarReceitaAtual();
  }
  new ResizeObserver(() => { if (!$("#rotular").hidden) desenhar(); }).observe($("#palco"));

  // Desenhar, escolher e apagar caixas
  let arrasto = null;
  const ponto = (ev) => {
    const r = tela.getBoundingClientRect();
    const x = (ev.clientX - r.left - enc.x) / enc.w; const y = (ev.clientY - r.top - enc.y) / enc.h;
    return { x: Math.min(1, Math.max(0, x)), y: Math.min(1, Math.max(0, y)), dentro: x >= 0 && x <= 1 && y >= 0 && y <= 1 };
  };
  tela.onpointerdown = (ev) => {
    const f = S.fotos[S.atual];
    if (!f || !cache.get(S.atual)) return;
    const p = ponto(ev);
    if (!p.dentro) return;
    // Clique numa caixa existente (a menor por cima) seleciona.
    const achou = f.caixas.map((c, i) => ({ c, i, a: Math.abs((c.x1 - c.x0) * (c.y1 - c.y0)) }))
      .filter(({ c }) => p.x >= Math.min(c.x0, c.x1) && p.x <= Math.max(c.x0, c.x1) && p.y >= Math.min(c.y0, c.y1) && p.y <= Math.max(c.y0, c.y1))
      .sort((a, b) => a.a - b.a)[0];
    if (achou) { S.sel = achou.i; S.classeAtiva = achou.c.c; montarClassesRotulo(); desenhar(); return; }
    if (!S.classes.length) { $("#foto-info").textContent = "Adicione os objetos no passo 1."; return; }
    S.sel = -1;
    tela.setPointerCapture(ev.pointerId);
    arrasto = { nova: { c: S.classeAtiva, x0: p.x, y0: p.y, x1: p.x, y1: p.y } };
  };
  tela.onpointermove = (ev) => {
    if (!arrasto) return;
    const p = ponto(ev);
    arrasto.nova.x1 = p.x; arrasto.nova.y1 = p.y;
    desenhar();
  };
  tela.onpointerup = () => {
    if (!arrasto) return;
    const c = arrasto.nova; arrasto = null;
    const f = S.fotos[S.atual];
    if (Math.abs(c.x1 - c.x0) > 0.004 && Math.abs(c.y1 - c.y0) > 0.004) {
      f.caixas.push({ c: c.c, x0: Math.min(c.x0, c.x1), y0: Math.min(c.y0, c.y1), x1: Math.max(c.x0, c.x1), y1: Math.max(c.y0, c.y1) });
      f.vazia = false;
      S.sel = f.caixas.length - 1;
    }
    desenhar();
  };
  tela.onpointercancel = () => { arrasto = null; desenhar(); };
  function apagarCaixa() {
    const f = S.fotos[S.atual];
    if (!f || S.sel < 0) return;
    f.caixas.splice(S.sel, 1);
    S.sel = -1;
    desenhar();
  }
  $("#apagar-caixa").onclick = apagarCaixa;
  $("#sem-objeto").onclick = () => {
    const f = S.fotos[S.atual];
    if (!f) return;
    f.vazia = !f.vazia;
    if (f.vazia) { f.caixas = []; S.sel = -1; }
    desenhar();
  };
  document.addEventListener("keydown", (ev) => {
    if ($("#rotular").hidden || ev.target.closest("input, select, textarea") || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key === "ArrowRight") { ev.preventDefault(); mostrarFoto(S.atual + 1); }
    else if (ev.key === "ArrowLeft") { ev.preventDefault(); mostrarFoto(S.atual - 1); }
    else if (ev.key === "Delete" || ev.key === "Backspace") { ev.preventDefault(); apagarCaixa(); }
    else if (ev.key === "Escape") { S.sel = -1; desenhar(); }
    else if (/^[1-9]$/.test(ev.key)) escolherClasse(Number(ev.key) - 1);
  });

  // Exportar no formato YOLO: images/ + labels/ + data.yaml, num .zip montado aqui.
  const CRC = (() => {
    const t = new Uint32Array(256);
    for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
    return t;
  })();
  function crc32(b) { let c = 0xffffffff; for (let i = 0; i < b.length; i++) c = CRC[(c ^ b[i]) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; }

  /** Zip mínimo, sem compressão ("store"), nomes em UTF-8. */
  function zip(arquivos) {
    const te = new TextEncoder();
    const partes = []; const central = []; let pos = 0;
    const d = new Date();
    const hora = (d.getHours() << 11) | (d.getMinutes() << 5) | (d.getSeconds() >> 1);
    const dia = ((d.getFullYear() - 1980) << 9) | ((d.getMonth() + 1) << 5) | d.getDate();
    for (const { nome, dados } of arquivos) {
      const n = te.encode(nome); const crc = crc32(dados);
      const loc = new DataView(new ArrayBuffer(30));
      loc.setUint32(0, 0x04034b50, true); loc.setUint16(4, 20, true); loc.setUint16(6, 0x0800, true);
      loc.setUint16(8, 0, true); loc.setUint16(10, hora, true); loc.setUint16(12, dia, true);
      loc.setUint32(14, crc, true); loc.setUint32(18, dados.length, true); loc.setUint32(22, dados.length, true);
      loc.setUint16(26, n.length, true); loc.setUint16(28, 0, true);
      partes.push(loc, n, dados);
      const cen = new DataView(new ArrayBuffer(46));
      cen.setUint32(0, 0x02014b50, true); cen.setUint16(4, 20, true); cen.setUint16(6, 20, true); cen.setUint16(8, 0x0800, true);
      cen.setUint16(10, 0, true); cen.setUint16(12, hora, true); cen.setUint16(14, dia, true);
      cen.setUint32(16, crc, true); cen.setUint32(20, dados.length, true); cen.setUint32(24, dados.length, true);
      cen.setUint16(28, n.length, true); cen.setUint32(42, pos, true);
      central.push(cen, n);
      pos += 30 + n.length + dados.length;
    }
    const tamCentral = central.reduce((s, p) => s + p.byteLength, 0);
    const fim = new DataView(new ArrayBuffer(22));
    fim.setUint32(0, 0x06054b50, true); fim.setUint16(8, arquivos.length, true); fim.setUint16(10, arquivos.length, true);
    fim.setUint32(12, tamCentral, true); fim.setUint32(16, pos, true);
    return new Blob([...partes, ...central, fim], { type: "application/zip" });
  }

  const n6 = (v) => Math.min(1, Math.max(0, v)).toFixed(6);
  $("#exportar").onclick = async () => {
    const botao = $("#exportar");
    botao.disabled = true;
    try {
      const te = new TextEncoder();
      const arquivos = []; const usados = new Set();
      for (const f of S.fotos) {
        if (!f.caixas.length && !f.vazia) continue;
        const ext = (f.nome.match(EXT) || [".jpg"])[0].toLowerCase();
        const base0 = f.nome.replace(/\.[^.]+$/, "").replace(/[^\w.-]+/g, "_").slice(0, 80) || "foto";
        let base = base0; let k = 2;
        while (usados.has(base)) base = `${base0}-${k++}`;
        usados.add(base);
        const linhas = f.caixas.map((b) => `${b.c} ${n6((b.x0 + b.x1) / 2)} ${n6((b.y0 + b.y1) / 2)} ${n6(b.x1 - b.x0)} ${n6(b.y1 - b.y0)}`);
        arquivos.push({ nome: `images/${base}${ext}`, dados: new Uint8Array(await f.arquivo.arrayBuffer()) });
        arquivos.push({ nome: `labels/${base}.txt`, dados: te.encode(linhas.join("\n") + (linhas.length ? "\n" : "")) });
      }
      const yaml = [
        "# CamFácil IA · modo estúdio · somente para estudo e aprendizagem",
        `# receita: ${idReceita()}`,
        ...S.classes.map((c, i) => `# ${i}: ${c.nome}`),
        "path: .", "train: images", "val: images",
        `nc: ${S.classes.length}`,
        `names: [${S.classes.map((c) => JSON.stringify(c.id)).join(", ")}]`, "",
      ].join("\n");
      arquivos.push({ nome: "data.yaml", dados: te.encode(yaml) });
      baixar(zip(arquivos), `${idReceita()}-fotos-rotuladas.zip`);
      $("#foto-info").textContent = `${arquivos.length / 2 | 0} fotos exportadas. Envie o .zip no Colab (passo Treinar).`;
      atualizarReceitaAtual();
    } finally { botao.disabled = false; }
  };

  // ---------------------------------------------------------------- 4. treinar
  function atualizarTreinar() {
    const n = fotosRotuladas();
    $("#treinar-situacao").textContent = !form.nome.value.trim() ? "Descreva a receita no passo 1 antes de baixar."
      : `Receita ${idReceita()} · ${S.classes.length} objeto${S.classes.length === 1 ? "" : "s"}` + (n ? ` · ${n} fotos rotuladas.` : ". Rotule pelo menos 20 fotos (o ideal é de 100 a 300).");
  }
  $("#baixar-notebook").onclick = async () => {
    const msg = $("#treinar-msg");
    msg.className = "msg"; msg.textContent = "";
    if (!(await conferir())) { msg.className = "msg err"; msg.textContent = "A receita não passou na conferência. Volte ao passo 1."; return; }
    const botao = $("#baixar-notebook");
    botao.disabled = true;
    try {
      const r = await fetch("/api/estudio/notebook", { method: "POST", credentials: "same-origin", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ receita: S.valida }) });
      if (!r.ok) { const d = await r.json().catch(() => ({})); throw new Error(d.erro || r.statusText); }
      const nome = (/filename="([^"]+)"/.exec(r.headers.get("content-disposition") || "") || [])[1] || `camfacil-estudio-${S.valida.id}.ipynb`;
      baixar(await r.blob(), nome);
      msg.className = "msg ok"; msg.textContent = `${nome} baixado. Agora abra o Colab e faça o upload dele.`;
    } catch (e) { msg.className = "msg err"; msg.textContent = e.message; }
    finally { botao.disabled = false; }
  };

  // ---------------------------------------------------------------- 6. compartilhar
  let logado = null; let receitas = []; let pagina = 0; let porPagina = 5;
  function escolherSub(nome) {
    $$(".seg-sub [data-sub]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.sub === nome)));
    $$("#compartilhar [data-subpainel]").forEach((p) => p.classList.toggle("sub-atual", p.dataset.subpainel === nome));
    if (nome === "galeria") requestAnimationFrame(paginar);
  }
  $$(".seg-sub [data-sub]").forEach((b) => { b.onclick = () => escolherSub(b.dataset.sub); });

  async function abrirCompartilhar() {
    if (!$(".seg-sub [aria-selected=true]")) escolherSub("publicar");
    else escolherSub($(".seg-sub [aria-selected=true]").dataset.sub);
    if (logado === null) {
      try { logado = await api("/api/eu"); } catch (_) { logado = false; }
    }
    $("#publicar-sair").hidden = !!logado;
    $("#form-publicar").hidden = !logado;
    buscar();
  }

  $("#form-publicar").onsubmit = async (ev) => {
    ev.preventDefault();
    const f = ev.target; const msg = $("#publicar-msg");
    msg.className = "msg"; msg.textContent = "";
    let receita;
    try {
      if (f.final.files[0]) receita = JSON.parse(await f.final.files[0].text());
      else { if (!(await conferir())) throw new Error("A receita não passou na conferência. Volte ao passo 1."); receita = S.valida; }
    } catch (e) { msg.className = "msg err"; msg.textContent = e instanceof SyntaxError ? "Esse arquivo não é um JSON válido." : e.message; return; }
    try {
      const r = await api("/api/estudio/receitas", "POST", { receita, autor: f.autor.value.trim(), publica: f.publica.checked });
      msg.className = "msg ok";
      msg.textContent = r.publica ? `Receita ${r.receita_id} publicada. Obrigado por ensinar outros técnicos.` : `Receita ${r.receita_id} guardada só para você.`;
      buscar();
    } catch (e) {
      msg.className = "msg err";
      msg.textContent = e.status === 401 ? "Sua sessão terminou. Entre de novo." : (e.erros && e.erros.length ? e.erros.slice(0, 2).join("; ") : e.message);
    }
  };

  let tBusca = 0;
  $("#busca").oninput = () => { clearTimeout(tBusca); tBusca = setTimeout(buscar, 250); };
  async function buscar() {
    const q = $("#busca").value.trim();
    try { receitas = (await api(`/api/estudio/receitas${q ? `?q=${encodeURIComponent(q)}` : ""}`)).receitas; }
    catch (_) { receitas = []; }
    pagina = 0;
    paginar();
  }
  function itemGaleria(r) {
    const usar = el("button", { class: "btn ghost sm", type: "button" }, "Usar");
    usar.onclick = async () => {
      try { const rec = await (await fetch(`/api/estudio/receitas/${r.id}/receita.json`)).json(); preencher(rec); ir(0, true); conferir(); }
      catch (_) { /* segue */ }
    };
    const extra = [r.classes.join(", "), r.autor ? `por ${r.autor}` : null, r.metricas && r.metricas.map50 != null ? `mAP50 ${Number(r.metricas.map50).toFixed(2)}` : null, `${r.downloads} download${r.downloads === 1 ? "" : "s"}`].filter(Boolean).join(" · ");
    return el("li", {}, el("div", {}, el("strong", {}, r.nome), el("span", { class: "tiny" }, extra)),
      el("div", { class: "actions" }, usar, el("a", { class: "btn sm", href: `/api/estudio/receitas/${r.id}/receita.json`, download: `${r.receita_id}.receita.json` }, "Baixar")));
  }
  function paginar() {
    const lista = $("#galeria");
    if (!receitas.length) {
      lista.replaceChildren(el("li", { class: "vazio" }, $("#busca").value.trim() ? "Nenhuma receita com esse nome ou objeto." : "Nenhuma receita publicada ainda. Seja o primeiro."));
      $(".gal-pag").hidden = true;
      return;
    }
    // Quantas cabem no espaço fixo: mede um item de verdade.
    lista.replaceChildren(itemGaleria(receitas[0]));
    const h = lista.firstChild.getBoundingClientRect().height || 52;
    porPagina = Math.max(1, Math.floor((lista.clientHeight - 2) / h));
    const total = Math.ceil(receitas.length / porPagina);
    pagina = Math.min(pagina, total - 1);
    lista.replaceChildren(...receitas.slice(pagina * porPagina, (pagina + 1) * porPagina).map(itemGaleria));
    $(".gal-pag").hidden = total < 2;
    $("#gal-pos").textContent = `${pagina + 1} de ${total}`;
    $("#gal-ant").disabled = pagina === 0; $("#gal-prox").disabled = pagina >= total - 1;
  }
  $("#gal-ant").onclick = () => { pagina--; paginar(); };
  $("#gal-prox").onclick = () => { pagina++; paginar(); };
  let tRes = 0;
  addEventListener("resize", () => { clearTimeout(tRes); tRes = setTimeout(() => { if (!$("#compartilhar").hidden) paginar(); }, 120); });

  // ---------------------------------------------------------------- início
  const inicial = lerRascunho();
  desenharChips();
  mostrarRegra();
  atualizarReceitaAtual();
  // Aplicação futura (só desenho): VMS sob medida.
  const abrirVms = () => {
    PASSOS.forEach((p) => { $("#" + p).hidden = true; });
    botoesPasso.forEach((b) => b.setAttribute("aria-selected", "false"));
    $("#vms").hidden = false; $(".est-nav").hidden = true;
    $("#abrir-vms").setAttribute("aria-current", "page");
    $("#t-vms").focus();
  };
  $("#t-vms").tabIndex = -1;
  $("#abrir-vms").onclick = abrirVms;
  $("#fechar-vms").onclick = () => ir(S.passo, true);
  if (location.hash === "#vms") setTimeout(abrirVms, 0);
  const pedido = PASSOS.indexOf(location.hash.slice(1));
  ir(pedido >= 0 ? pedido : inicial);
})();

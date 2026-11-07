// Formação de IA para CFTV · tudo roda no navegador, sem servidor e sem câmera.
// Cena 960×540 (foto da loja). A pessoa tem 126×250; o "centro" que a IA
// segue fica em (63, 125) dentro do sprite.
"use strict";

const $ = (s, r = document) => r.querySelector(s);
const el = (tag, attrs = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v; else if (k.startsWith("on")) e[k] = v; else e.setAttribute(k, v);
  }
  e.append(...kids.filter((k) => k != null));
  return e;
};
const NS = "http://www.w3.org/2000/svg";
const svgEl = (tag, attrs = {}) => {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  return e;
};

// ---------- Cena base ----------
const W = 960, H = 540;
const SPRITE = { w: 126, h: 250, cx: 63, cy: 125 };
// Trajeto de quem entra: aparece na porta (longe, pequeno) e vem em direção
// à câmera (perto, grande), saindo cortado pela borda de baixo/esquerda.
// A porta dupla de vidro fica no fundo (x 245–430, soleira em y≈300). A pessoa
// aparece pequena na calçada, vista pelo vidro (pés em y≈240), passa pela
// soleira e vem reto pelo corredor até a câmera, crescendo.
const PORTA = { x: 326, y: 165, s: 0.3 }; // pés em (345, 240)
const PERTO = { x: 292, y: 350, s: 1.4 }; // pés em (380, 700): já saiu por baixo
// Onde o cliente para para ser atendido: na frente do balcão (pés em 505, 495).
const BALCAO = { x: 442, y: 245, s: 1.0 };
// Faixa da borda onde a IA ainda vê a pessoa cortada (≈ uma pessoa de largura).
const MARGEM = 0.09;

const lerp = (a, b, k) => a + (b - a) * k;
function noTrajeto(k, de = PORTA, para = PERTO, dx = 0) {
  return { x: lerp(de.x, para.x, k) + dx, y: lerp(de.y, para.y, k), s: lerp(de.s, para.s, k) };
}

function cena(cls, conteudo = "", margem = false) {
  const faixa = margem ? `<g class="margem"><path fill-rule="evenodd" d="M0 0H${W}V${H}H0Z M${W * MARGEM} ${H * MARGEM}H${W * (1 - MARGEM)}V${H * (1 - MARGEM)}H${W * MARGEM}Z"/><rect class="util" x="${W * MARGEM}" y="${H * MARGEM}" width="${W * (1 - 2 * MARGEM)}" height="${H * (1 - 2 * MARGEM)}" rx="6"/><text class="margem-t" x="${W / 2}" y="${H - 16}">faixa da borda: aqui a IA vê a pessoa cortada</text><text class="util-t" x="${W * MARGEM + 14}" y="${H * MARGEM + 34}">área útil: desenhe aqui</text></g>` : "";
  return `<svg class="cena ${cls}" viewBox="0 0 ${W} ${H}" role="img">
    <defs>
      <clipPath id="clip-${cls}"><rect width="${W}" height="${H}" rx="14"/></clipPath>
      <linearGradient id="g-${cls}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#06cb3f" stop-opacity="0"/><stop offset="1" stop-color="#06cb3f" stop-opacity=".18"/></linearGradient>
    </defs>
    <g clip-path="url(#clip-${cls})">
      <image href="/img/loja.webp" width="${W}" height="${H}"/>
      ${conteudo}
      ${faixa}
      <g class="hud"><path d="M18 58V18h40M902 18h40v40M942 482v40h-40M58 522H18v-40"/><circle cx="40" cy="40" r="6" class="rec"/><text x="54" y="46">REC · CAM 01</text></g>
      <rect class="varredura" x="0" y="0" width="${W}" height="70" fill="url(#g-${cls})"/>
    </g>
    <rect class="moldura" x="1" y="1" width="${W - 2}" height="${H - 2}" rx="14"/>
  </svg>`;
}
// Quem entra vem de frente para a câmera; quem sai, de costas.
const pessoaSvg = (lado = "frente") => `<g class="pessoa"><image href="/img/pessoa-${lado}.webp" width="${SPRITE.w}" height="${SPRITE.h}"/><g class="centro"><circle cx="63" cy="125" r="7"/><circle cx="63" cy="125" r="16" class="anel"/><path d="M63 97v14M63 139v14M35 125h14M77 125h14"/></g></g>`;
const PESSOA = pessoaSvg("frente");

function painel(cls, ok, titulo, legenda, conteudo, margem = false) {
  return `<figure class="anim-painel ${cls}"><div class="anim-tit ${ok ? "ok" : "erro"}">${titulo}</div>${cena(cls, conteudo, margem)}<figcaption>${legenda}</figcaption></figure>`;
}

const ANIM = {
  visao: () => `<div class="anim-grade um">${painel("visao", true, "Como a IA enxerga",
    "Ela não reconhece “uma pessoa”: percebe o que se mexeu em relação ao fundo e acompanha o <strong>centro</strong> dessa mancha (a mira verde). Repare: quando a pessoa sai cortada pela borda, a mira some. Toda regra olha para esse centro.", PESSOA)}</div>`,
  margem: () => `<div class="anim-grade um">${painel("faixa", false, "A faixa da borda",
    "Em volta da imagem há uma faixa, mais ou menos a largura de uma pessoa (uns 10%), onde a IA só vê a pessoa <strong>cortada</strong>: entrando ou saindo do quadro. Ali o centro não existe, e nada que você desenhar vai funcionar. Desenhe sempre dentro da área útil.", PESSOA, true)}</div>`,
  linha: () => `<div class="anim-grade">${painel("borda", false, "Errado: linha na borda",
    "A pessoa chega à linha já <strong>cortada</strong> pela borda de baixo. A IA perdeu o centro antes de ela cruzar. Não conta.",
    `<path class="linha erro" d="M60 498H700"/>${PESSOA}<text class="placar" x="930" y="110">passagens: 0</text><text class="aviso" x="930" y="160">não contou</text>`)}
    ${painel("meio", true, "Certo: linha atravessando o corredor",
    "A linha cruza o caminho na frente da porta, longe das bordas. A IA vê o centro <strong>antes</strong> e <strong>depois</strong>. Conta.",
    `<path class="linha ok" d="M250 352L700 322"/>${PESSOA}<text class="placar p0" x="930" y="110">passagens: 0</text><text class="placar p1" x="930" y="110">passagens: 1</text>`)}</div>`,
  zona: () => `<div class="anim-grade">${painel("zgrande", false, "Errado: zona pegando o corredor",
    "Quem só passa pelo corredor entra na zona e dispara o aviso: <strong>alarme falso</strong>. O cliente perde a confiança.",
    `<path class="zona" d="M120 290H900V530H40Z"/>${PESSOA}<text class="aviso za" x="480" y="110">alarme falso</text>`)}
    ${painel("zcerta", true, "Certo: só na frente do balcão",
    "Quem passa não dispara. Só quem <strong>para no balcão</strong> conta como ocupado.",
    `<path class="zona" d="M430 285H610V525H430Z"/><g class="passante">${PESSOA}</g><g class="cliente">${PESSOA}</g><text class="okt zc" x="520" y="262">ocupado</text>`)}</div>`,
};

// ---------- Som do alerta (gerado na hora) ----------
let somCtx = null;
function bip() {
  try {
    somCtx = somCtx || new (window.AudioContext || window.webkitAudioContext)();
    const t0 = somCtx.currentTime;
    for (const [i, f] of [[0, 880], [0.18, 1320]]) {
      const o = somCtx.createOscillator(); const g = somCtx.createGain();
      o.type = "sine"; o.frequency.value = f;
      g.gain.setValueAtTime(0.0001, t0 + i);
      g.gain.exponentialRampToValueAtTime(0.25, t0 + i + 0.02);
      g.gain.exponentialRampToValueAtTime(0.0001, t0 + i + 0.16);
      o.connect(g).connect(somCtx.destination);
      o.start(t0 + i); o.stop(t0 + i + 0.18);
    }
  } catch (_) { /* sem áudio */ }
}

// ---------- Geometria ----------
function cruzou(a, b, p, q) {
  // Segmentos a→b e p→q se cruzam?
  const d = (o, s, t) => (s.x - o.x) * (t.y - o.y) - (s.y - o.y) * (t.x - o.x);
  const d1 = d(p, q, a), d2 = d(p, q, b), d3 = d(a, b, p), d4 = d(a, b, q);
  return ((d1 > 0) !== (d2 > 0)) && ((d3 > 0) !== (d4 > 0));
}
const dentro = (pt, r) => pt.x >= r.x && pt.x <= r.x + r.w && pt.y >= r.y && pt.y <= r.y + r.h;

function coordSvg(svg, ev) {
  const r = svg.getBoundingClientRect();
  return { x: ((ev.clientX - r.left) / r.width) * W, y: ((ev.clientY - r.top) / r.height) * H };
}

// Pessoa controlada por código (rAF). Devolve o centro quando a IA "vê" a pessoa inteira.
function criarPessoa(svg, lado = "frente") {
  const g = svgEl("g", { class: "pessoa" });
  g.innerHTML = pessoaSvg(lado).replace(/^<g class="pessoa">|<\/g>$/g, "");
  svg.querySelector(".atores").append(g);
  const mira = g.querySelector(".centro");
  return {
    g,
    mover(x, y, escala = 1) {
      g.setAttribute("transform", `translate(${x} ${y}) scale(${escala})`);
      // A IA só segue o centro dentro da área útil; na faixa da borda a pessoa
      // está cortada e o centro não existe.
      const c = { x: x + SPRITE.cx * escala, y: y + SPRITE.cy * escala };
      const inteira = c.x >= W * MARGEM && c.x <= W * (1 - MARGEM) && c.y >= H * MARGEM && c.y <= H * (1 - MARGEM);
      mira.style.opacity = inteira ? 1 : 0;
      return inteira ? c : null;
    },
    posicionar(p) { return this.mover(p.x, p.y, p.s); },
    remover() { g.remove(); },
  };
}

function animar(duracaoMs, passo) {
  return new Promise((fim) => {
    const t0 = performance.now();
    const tick = (t) => {
      const k = Math.min(1, (t - t0) / duracaoMs);
      passo(k);
      if (k < 1) requestAnimationFrame(tick); else fim();
    };
    requestAnimationFrame(tick);
  });
}

// ---------- Narração de acerto e erro ----------
function falar(id) { if (typeof tocar === "function" && temNarracao) tocar(id); }

// ---------- Exercício: linha ----------
function exercicioLinha(raiz, concluir) {
  raiz.innerHTML = `
    <div class="exercicio">
      <div class="ex-topo"><strong>Sua vez: desenhe a linha de passagem</strong><p class="hint">Dois cliques na imagem, atravessando o corredor na frente da porta. Depois passam três pessoas: duas entram e uma sai. A IA conta e diz se a sua linha pegou todo mundo.</p></div>
      ${cena("ex-linha", `<g class="formas"></g><g class="atores"></g><text class="placar" x="930" y="110">entraram: 0 · saíram: 0</text>`, true)}
      <div class="ex-acoes">
        <button class="btn" type="button" data-apagar><svg class="ico sm"><use href="#i-redo"/></svg><span>Apagar e tentar de novo</span></button>
        <label class="som-opcao"><span class="switch"><input type="checkbox" checked data-faixa><span></span></span><span class="tiny">Mostrar faixa da borda</span></label>
        <span class="tiny" data-dica>Clique na imagem para começar a linha.</span>
      </div>
      <p class="resultado" data-resultado role="status"></p>
    </div>`;
  const svg = $("svg", raiz); const formas = $(".formas", svg); const placar = $(".placar", svg);
  const dica = $("[data-dica]", raiz); const res = $("[data-resultado]", raiz);
  $("[data-faixa]", raiz).onchange = (ev) => { $(".margem", svg).style.display = ev.target.checked ? "" : "none"; };
  let pontos = []; let linha = null; let rodando = false; let feito = false;

  const desenhar = (fim) => {
    formas.replaceChildren();
    if (!pontos.length) return;
    const pts = [...pontos, fim].filter(Boolean);
    linha = svgEl("polyline", { class: "linha-usuario", points: pts.map((p) => `${p.x},${p.y}`).join(" ") });
    formas.append(linha);
    for (const p of pontos) formas.append(svgEl("circle", { class: "vertice", cx: p.x, cy: p.y, r: 9 }));
  };
  const limpar = () => { pontos = []; formas.replaceChildren(); placar.textContent = "entraram: 0 · saíram: 0"; res.textContent = ""; res.className = "resultado"; dica.textContent = "Clique na imagem para começar a linha."; };

  svg.addEventListener("pointermove", (ev) => { if (pontos.length === 1 && !rodando) desenhar(coordSvg(svg, ev)); });
  svg.addEventListener("pointerdown", (ev) => {
    if (rodando) return;
    if (pontos.length >= 2) limpar();
    pontos.push(coordSvg(svg, ev));
    desenhar(null);
    if (pontos.length === 1) dica.textContent = "Agora o segundo ponto.";
    if (pontos.length === 2) { dica.textContent = "Olha as pessoas passando…"; simular(); }
  });
  $("[data-apagar]", raiz).onclick = () => { if (!rodando) limpar(); };

  // Três passagens: entra (centro do corredor), entra (mais à esquerda), sai.
  const PASSAGENS = [
    { tipo: "entrou", de: PORTA, para: PERTO, dx: 0 },
    { tipo: "entrou", de: PORTA, para: PERTO, dx: -55 },
    { tipo: "saiu", de: PERTO, para: PORTA, dx: 40, lado: "costas" },
  ];

  async function simular() {
    rodando = true;
    const [a, b] = pontos;
    const conta = { entrou: 0, saiu: 0 };
    for (const pas of PASSAGENS) {
      const pessoa = criarPessoa(svg, pas.lado);
      let anterior = null; let contouEsta = false;
      await animar(4200, (k) => {
        const c = pessoa.posicionar(noTrajeto(k, pas.de, pas.para, pas.dx));
        if (c && anterior && !contouEsta && cruzou(anterior, c, a, b)) {
          contouEsta = true; conta[pas.tipo] += 1;
          placar.textContent = `entraram: ${conta.entrou} · saíram: ${conta.saiu}`;
          linha.classList.add("flash"); setTimeout(() => linha.classList.remove("flash"), 500);
        }
        anterior = c || anterior;
      });
      pessoa.remover();
    }
    rodando = false;
    avaliar(a, b, conta);
  }

  function avaliar(a, b, conta) {
    const total = conta.entrou + conta.saiu;
    const mx = W * MARGEM, my = H * MARGEM;
    const naBorda = Math.max(a.x, b.x) < mx || Math.min(a.x, b.x) > W - mx || Math.max(a.y, b.y) < my || Math.min(a.y, b.y) > H - my;
    if (total === 3) {
      res.className = "resultado ok";
      res.textContent = "Contou tudo: 2 entraram, 1 saiu. A linha atravessa o corredor inteiro, longe das bordas. É assim que se desenha.";
      dica.textContent = "Pode seguir para a próxima lição.";
      falar("escola-acerto-linha");
      if (!feito) { feito = true; concluir(); }
    } else if (total > 0) {
      res.className = "resultado erro";
      res.textContent = `Contou só ${total} de 3. Alguém passou ao lado da linha. Faça a linha mais comprida, atravessando o corredor de parede a balcão.`;
      falar("escola-erro-curta");
    } else if (naBorda) {
      res.className = "resultado erro";
      res.textContent = "Não contou. A linha está na faixa da borda: ali a IA só vê a pessoa cortada e o centro não existe. Apague e desenhe mais para o meio, na frente da porta.";
      falar("escola-erro-borda");
    } else {
      res.className = "resultado erro";
      res.textContent = "Não contou. A linha não cruza o caminho: as pessoas entram pela porta e vêm em direção à câmera. Desenhe atravessando o corredor, da parede até o balcão.";
      falar("escola-erro-caminho");
    }
  }
}

// ---------- Exercício: zona ----------
function exercicioZona(raiz, concluir) {
  raiz.innerHTML = `
    <div class="exercicio">
      <div class="ex-topo"><strong>Sua vez: desenhe a zona do balcão</strong><p class="hint">Clique e arraste na imagem para marcar um retângulo na frente do balcão. Depois passam duas pessoas pelo corredor e uma entra e para para ser atendida.</p></div>
      ${cena("ex-zona", `<g class="formas"></g><g class="atores"></g><text class="aviso za" x="480" y="110"></text>`)}
      <div class="ex-acoes">
        <button class="btn" type="button" data-apagar><svg class="ico sm"><use href="#i-redo"/></svg><span>Apagar e tentar de novo</span></button>
        <span class="tiny" data-dica>Arraste na imagem para desenhar.</span>
      </div>
      <p class="resultado" data-resultado role="status"></p>
    </div>`;
  const svg = $("svg", raiz); const formas = $(".formas", svg); const aviso = $(".aviso", svg);
  const dica = $("[data-dica]", raiz); const res = $("[data-resultado]", raiz);
  let ini = null; let rect = null; let zona = null; let rodando = false; let feito = false;

  const norm = (p, q) => ({ x: Math.min(p.x, q.x), y: Math.min(p.y, q.y), w: Math.abs(q.x - p.x), h: Math.abs(q.y - p.y) });
  const desenhar = () => {
    formas.replaceChildren();
    if (!rect) return;
    zona = svgEl("rect", { class: "zona-usuario", x: rect.x, y: rect.y, width: rect.w, height: rect.h, rx: 6 });
    formas.append(zona);
  };
  const mostrar = (txt, cls) => { aviso.textContent = txt; aviso.setAttribute("class", `aviso za ${cls || ""}`); };
  const limpar = () => { ini = null; rect = null; formas.replaceChildren(); mostrar(""); res.textContent = ""; res.className = "resultado"; dica.textContent = "Arraste na imagem para desenhar."; };

  svg.addEventListener("pointerdown", (ev) => { if (rodando) return; if (rect) limpar(); ini = coordSvg(svg, ev); svg.setPointerCapture(ev.pointerId); });
  svg.addEventListener("pointermove", (ev) => { if (!ini || rodando) return; rect = norm(ini, coordSvg(svg, ev)); desenhar(); });
  svg.addEventListener("pointerup", (ev) => {
    if (!ini || rodando) return;
    rect = norm(ini, coordSvg(svg, ev)); ini = null;
    if (rect.w < 40 || rect.h < 40) { rect = null; formas.replaceChildren(); dica.textContent = "Ficou pequena demais. Arraste um pouco mais."; return; }
    desenhar(); dica.textContent = "Olha quem passa e quem para…"; simular();
  });
  $("[data-apagar]", raiz).onclick = () => { if (!rodando) limpar(); };

  async function simular() {
    rodando = true;
    let falso = false; let ocupado = false;
    // 1) dois passantes: um entra, outro sai
    for (const [de, para, dx, lado] of [[PORTA, PERTO, 0, "frente"], [PERTO, PORTA, -50, "costas"]]) {
      const p = criarPessoa(svg, lado);
      await animar(3600, (k) => {
        const c = p.posicionar(noTrajeto(k, de, para, dx));
        if (c && dentro(c, rect)) { falso = true; zona.classList.add("falso"); mostrar("alarme falso", "erro"); }
      });
      p.remover();
      zona.classList.remove("falso");
    }
    // 2) cliente: entra pela porta e para na frente do balcão
    const p2 = criarPessoa(svg);
    await animar(2600, (k) => {
      const ease = 1 - Math.pow(1 - k, 3);
      const c = p2.posicionar(noTrajeto(ease, PORTA, BALCAO));
      if (c && dentro(c, rect)) { ocupado = true; zona.classList.add("ocupado"); if (!falso) mostrar("ocupado", "ok"); }
    });
    await animar(1600, () => {});
    p2.remover();
    zona.classList.remove("ocupado");
    rodando = false;
    avaliar(falso, ocupado);
  }

  function avaliar(falso, ocupado) {
    if (falso) {
      res.className = "resultado erro";
      res.textContent = "Alarme falso: a zona pegou o corredor, e quem só estava passando disparou o aviso. Apague e desenhe só em volta da frente do balcão, sem pegar a passagem.";
      falar("escola-erro-zona-falso");
    } else if (ocupado) {
      res.className = "resultado ok";
      res.textContent = "Certo! Quem passou pelo corredor não disparou e quem parou no balcão marcou ocupado. Zona justa, só onde a pessoa fica.";
      dica.textContent = "Pode seguir para a próxima lição.";
      falar("escola-acerto-zona");
      if (!feito) { feito = true; concluir(); }
    } else {
      res.className = "resultado erro";
      res.textContent = "Ninguém disparou: a zona não cobre o lugar onde a pessoa parou, na frente do balcão. Desenhe o retângulo em volta dela.";
      falar("escola-erro-zona-vazia");
    }
  }
}

// ---------- Exercício: balões e alerta ----------
function exercicioAlerta(raiz, concluir) {
  raiz.innerHTML = `
    <div class="exercicio">
      <div class="ex-topo"><strong>O que o cliente vê</strong><p class="hint">Nada de caixas nem riscos: só os balões com o nome de cada regra. Clique em Simular e repare no balão, na tela e no som.</p></div>
      ${cena("ex-alerta", `<g class="atores"></g>
        <g class="balao b-porta"><rect x="560" y="150" width="140" height="44" rx="22"/><text x="630" y="179">Porta</text></g>
        <g class="balao b-balcao"><rect x="640" y="250" width="210" height="44" rx="22"/><text x="745" y="279">Balcão · livre</text></g>
        <rect class="flash-tela" width="${W}" height="${H}"/>`)}
      <div class="ex-acoes">
        <button class="btn primary" type="button" data-simular><svg class="ico sm"><use href="#i-play"/></svg><span>Simular uma pessoa entrando</span></button>
        <label class="som-opcao"><span class="switch"><input type="checkbox" checked data-som><span></span></span><span class="tiny">Som do alerta</span></label>
      </div>
      <p class="resultado" data-resultado role="status"></p>
    </div>`;
  const svg = $("svg", raiz); const porta = $(".b-porta", svg); const balcao = $(".b-balcao", svg); const flash = $(".flash-tela", svg);
  const res = $("[data-resultado]", raiz); const btn = $("[data-simular]", raiz); const som = $("[data-som]", raiz);
  const LINHA = [{ x: 250, y: 352 }, { x: 700, y: 322 }];
  let feito = false;
  btn.onclick = async () => {
    btn.disabled = true; res.textContent = ""; res.className = "resultado";
    const p = criarPessoa(svg);
    p.g.querySelector(".centro").style.display = "none"; // o cliente não vê a mira
    let anterior = null; let disparou = false;
    await animar(3600, (k) => {
      const c = p.posicionar(noTrajeto(k * 0.55, PORTA, PERTO));
      if (c && anterior && !disparou && cruzou(anterior, c, LINHA[0], LINHA[1])) {
        disparou = true;
        porta.classList.add("alerta"); flash.classList.add("pisca");
        $("text", porta).textContent = "Porta · passou";
        if (som.checked) bip();
        setTimeout(() => { porta.classList.remove("alerta"); flash.classList.remove("pisca"); $("text", porta).textContent = "Porta"; }, 2600);
      }
      anterior = c || anterior;
    });
    // vira e chega ao balcão
    const meio = noTrajeto(0.55, PORTA, PERTO);
    await animar(1600, (k) => { const e = 1 - Math.pow(1 - k, 2); p.posicionar(noTrajeto(e, meio, BALCAO)); });
    balcao.classList.add("ocupado"); $("text", balcao).textContent = "Balcão · ocupado";
    await animar(1800, () => {});
    balcao.classList.remove("ocupado"); $("text", balcao).textContent = "Balcão · livre";
    p.remover();
    btn.disabled = false;
    res.className = "resultado ok";
    res.textContent = "Foi isso que o cliente viu: o balão da porta ficou vermelho e piscou, a tela deu um clarão e tocou o bipe. O balcão marcou ocupado enquanto tinha gente. Nenhuma caixa em cima das pessoas.";
    if (!feito) { feito = true; concluir(); }
  };
}

// ---------- Exercício: saúde da câmera ----------
function exercicioSaude(raiz, concluir) {
  const casos = [
    { id: "ok", nome: "Normal", cls: "", diz: "A IA não reclama: imagem com luz e foco." },
    { id: "escura", nome: "Muito escura", cls: "f-escura", diz: "A IA avisa “câmera escura”: luz apagada ou infravermelho desligado." },
    { id: "coberta", nome: "Coberta", cls: "f-coberta", diz: "A IA avisa “câmera coberta”: alguém tapou a lente ou ela virou para a parede." },
    { id: "desfocada", nome: "Desfocada", cls: "f-desfocada", diz: "A IA avisa “câmera desfocada”: lente suja, chuva ou foco desregulado." },
  ];
  raiz.innerHTML = `
    <div class="exercicio">
      <div class="ex-topo"><strong>Qual câmera está desfocada?</strong><p class="hint">A IA olha a imagem o tempo todo e avisa quando a câmera não está enxergando direito. Clique na que está com problema de foco.</p></div>
      <div class="saude-grade" data-grade></div>
      <p class="resultado" data-resultado role="status"></p>
    </div>`;
  const grade = $("[data-grade]", raiz); const res = $("[data-resultado]", raiz);
  let feito = false;
  for (const c of casos) {
    const b = el("button", { class: `saude-caso ${c.cls}`, type: "button", "aria-label": c.nome });
    b.innerHTML = `<span class="saude-img"><img src="/img/loja.webp" alt=""><span class="saude-rotulo">CAM 0${casos.indexOf(c) + 1}</span></span>`;
    b.onclick = () => {
      grade.querySelectorAll(".saude-caso").forEach((x) => x.classList.remove("certo", "errado"));
      const acertou = c.id === "desfocada";
      b.classList.add(acertou ? "certo" : "errado");
      res.className = `resultado ${acertou ? "ok" : "erro"}`;
      res.textContent = (acertou ? "Isso. " : `Essa é a ${c.nome.toLowerCase()}. `) + c.diz + (acertou ? " No programa, esse aviso aparece no balão da câmera e na lista de eventos." : " Tente outra.");
      if (acertou) falar("escola-acerto-saude");
      if (acertou && !feito) { feito = true; concluir(); }
    };
    grade.append(b);
  }
}

// ---------- Lições ----------
const LICOES = [
  {
    id: "bemvindo", titulo: "Bem-vindo à formação", audio: "escola-00-abertura", abertura: true,
    texto: [
      "Obrigado por estar aqui. Se você chegou até esta página, é porque quer aprender de verdade, e isso já te coloca na frente de muita gente que só reclama que a IA é complicada.",
      "O que você vai aprender aqui é a inteligência de vídeo básica, primária: cruzamento de linha e cerca virtual. É o nível mais simples que existe, e mesmo assim resolve muito problema do dia a dia: saber quem entrou, quem passou onde não devia, quem parou no balcão. Só que ela tem regras. Desenhou errado, ela não funciona, e o cliente perde a confiança. Essas regras são o que eu vou te mostrar, na prática, em uns cinco minutos.",
      "E tem um detalhe importante: essa inteligência é muito leve. Roda em qualquer computador, roda em TV box, roda num celular velho. Aquele hardware parado no estoque, ou na casa do cliente, vira ferramenta de trabalho hoje, sem comprar nada.",
      "Esse é o primeiro passo. Você instala, prova que funciona, mostra que aquilo gera valor com custo quase zero. Quando o cliente vê o resultado e passa a acreditar em você, aí sim é hora de evoluir: câmera com inteligência de borda, gravador com IA, plataforma mais avançada. Primeiro a confiança, depois o investimento.",
      "Então vamos começar pelo começo: como a IA enxerga.",
    ],
  },
  {
    id: "visao", titulo: "Como a IA enxerga", audio: "escola-01",
    texto: [
      "Antes de desenhar qualquer coisa, entenda o que a inteligência artificial faz com a imagem. Ela não reconhece rosto, não sabe quem é a pessoa e não lê placa. Ela compara cada quadro com o fundo da cena, percebe o que se mexeu e acompanha o centro dessa mancha. É a mira verde que você vê na pessoa.",
      "Toda regra que você vai aprender aqui olha para esse centro: a linha conta quando o centro cruza, a zona avisa quando o centro está dentro. Guarde isso, porque é o segredo para desenhar certo.",
    ],
    anim: "visao",
  },
  {
    id: "linha", titulo: "A linha de passagem", audio: "escola-02",
    texto: [
      "A linha de passagem conta quem atravessa: porta de entrada, corredor, portão. Antes de desenhar, conheça a faixa da borda: em volta da imagem há uma margem, mais ou menos a largura de uma pessoa (uns 10%), onde a IA só vê a pessoa cortada, entrando ou saindo do quadro. Ali o centro não existe. Por isso, linha encostada na borda não conta nada.",
      "Regra de ouro: linha atravessando o corredor inteiro, de parede a balcão, no meio do caminho, dentro da área útil.",
    ],
    anim: ["margem", "linha"],
    exercicio: exercicioLinha,
  },
  {
    id: "zona", titulo: "A zona", audio: "escola-03",
    texto: [
      "A zona avisa quando tem alguém dentro dela: frente do balcão, caixa, área restrita. O erro aqui é o contrário da linha: desenhar grande demais. Se a zona pega o corredor, quem só está passando dispara o aviso. É alarme falso, e depois de três alarmes falsos o cliente para de olhar.",
      "Regra de ouro: zona justa, só onde a pessoa fica parada, bem na frente do balcão. Sem pegar a passagem.",
    ],
    anim: "zona",
    exercicio: exercicioZona,
  },
  {
    id: "alerta", titulo: "Balões, alertas e som", audio: "escola-04",
    texto: [
      "Para o cliente, nada de caixas, riscos ou máscaras em cima das pessoas. Ele vê só os balões com o nome de cada regra, do jeito que você escreveu. Quando uma regra dispara, o balão muda de cor e pisca, a tela dá um clarão e, se você deixar ligado, toca um bipe. Cada regra tem a própria chave de som.",
      "É esse visual limpo que faz o cliente entender na hora o que aconteceu, sem precisar de você do lado.",
    ],
    exercicio: exercicioAlerta,
  },
  {
    id: "saude", titulo: "Saúde da câmera", audio: "escola-05",
    texto: [
      "Câmera que ninguém olha estraga em silêncio: a luz queima, a aranha faz teia na lente, alguém vira a câmera para a parede. A IA fica de olho na própria imagem e avisa três coisas: câmera escura, câmera coberta e câmera desfocada.",
      "No programa, isso vira um balão e um evento, e pode virar mensagem no seu celular. É manutenção preventiva de graça.",
    ],
    exercicio: exercicioSaude,
  },
  {
    id: "pratica", titulo: "Da formação para a loja", audio: "escola-06",
    texto: [
      "Você já sabe o que importa: a IA segue o centro, a linha vai no meio do caminho, a zona fica só onde a pessoa para, o cliente vê só balões, e a câmera avisa quando não enxerga. Agora é operar.",
      "Baixe o programa no computador que está na mesma rede das câmeras. Ele abre direto em Câmeras, procura tudo na rede, você digita a senha e a imagem aparece. Depois, em cada câmera, desenhe as regras ou peça: IA, sugira para mim. Ela observa um minuto e propõe a linha e a zona. Você só aceita.",
      "E gere o seu selo: entre com a sua conta Google, coloque o seu Instagram se quiser ser repostado, e compartilhe.",
    ],
    final: true,
  },
];

// ---------- Progresso (neste navegador) ----------
const CHAVE = "cf-escola";
function progresso() { try { return JSON.parse(localStorage.getItem(CHAVE) || "{}"); } catch (_) { return {}; } }
function salvar(p) { try { localStorage.setItem(CHAVE, JSON.stringify(p)); } catch (_) { /* opcional */ } }
function feita(id) { return !!(progresso().feitas || {})[id]; }
function marcar(id) { const p = progresso(); p.feitas = p.feitas || {}; p.feitas[id] = true; salvar(p); desenharTrilha(); }

// ---------- Narração ----------
const voz = new Audio();
let vozBotao = null; let vozPendente = null;
function vozFalando(on) { $("#ouvir-caixa").classList.toggle("falando", on); $("#ouvir use").setAttribute("href", on ? "#i-pause" : "#i-play"); }
voz.onplaying = () => vozFalando(true);
voz.onended = () => { vozFalando(false); if (vozPendente) { const p = vozPendente; vozPendente = null; tocar(p, true); } };
voz.onpause = () => vozFalando(false);
voz.onerror = () => vozFalando(false);
function tocar(id, forcar = false) {
  if (!id) return;
  if (!voz.paused && !voz.ended && !forcar) { vozPendente = id; return; } // nunca corta a fala
  voz.src = `/audio/${id}.ogg`;
  voz.play().catch(() => vozFalando(false));
}
async function narracaoDisponivel() {
  try { return (await fetch("/audio/escola-01.ogg", { method: "HEAD" })).ok; } catch (_) { return false; }
}

// ---------- Tela ----------
let atual = 0;
let temNarracao = false;

function desenharTrilha() {
  const feitas = progresso().feitas || {};
  $("#trilha").replaceChildren(...LICOES.map((l, i) => {
    const ok = !!feitas[l.id];
    const b = el("button", { type: "button", class: `trilha-item${i === atual ? " atual" : ""}${ok ? " feita" : ""}`, onclick: () => ir(i) },
      el("span", { class: "num" }, ok ? "✓" : l.abertura ? "•" : String(i)),
      el("span", {}, l.titulo));
    return el("li", {}, b);
  }));
  const licoes = LICOES.filter((l) => !l.abertura);
  const n = licoes.filter((l) => feitas[l.id]).length;
  $("#trilha-progresso").textContent = n === licoes.length ? "Formação concluída. Selo conquistado!" : `${n} de ${licoes.length} lições feitas · progresso salvo neste navegador`;
}

function ir(i) {
  atual = Math.max(0, Math.min(LICOES.length - 1, i));
  const l = LICOES[atual];
  const p = progresso(); p.atual = atual; salvar(p);
  history.replaceState(null, "", `/aprender#${l.id}`);
  const n = LICOES.filter((x) => !x.abertura).length;
  $("#licao-num").textContent = l.abertura ? "Apresentação" : `Lição ${atual} de ${n}`;
  $("#licao-titulo").textContent = l.titulo;
  const corpo = $("#licao-corpo");
  corpo.replaceChildren(...l.texto.map((t) => el("p", { class: "licao-texto" }, t)));
  for (const nome of [].concat(l.anim || [])) { const a = el("div", { class: "aula-anim" }); a.innerHTML = ANIM[nome](); corpo.append(a); }
  const prox = $("#proxima"); const trava = $("#licao-trava");
  const liberar = () => { prox.disabled = false; trava.textContent = ""; marcar(l.id); };
  if (l.exercicio) {
    const ex = el("div"); corpo.append(ex);
    l.exercicio(ex, liberar);
    if (feita(l.id)) { prox.disabled = false; trava.textContent = "Já feita. Pode refazer ou seguir."; }
    else { prox.disabled = true; trava.textContent = "Faça o exercício para liberar a próxima."; }
  } else if (l.final) {
    marcar(l.id);
    corpo.append(telaFinal());
    prox.disabled = true; trava.textContent = "";
  } else {
    marcar(l.id);
    prox.disabled = false; trava.textContent = "";
  }
  $("#voltar").disabled = atual === 0;
  $("#proxima").hidden = !!l.final;
  $("#proxima span").textContent = l.abertura ? "Começar a lição 1" : "Próxima lição";
  $("#ouvir-caixa").hidden = !temNarracao || !l.audio;
  if (temNarracao && l.audio) tocar(l.audio);
  desenharTrilha();
  window.scrollTo({ top: 0, behavior: "instant" });
}

function telaFinal() {
  const feitas = progresso().feitas || {};
  const tudo = LICOES.every((l) => feitas[l.id]);
  const caixa = el("div", { class: "final" });
  const area = el("div", { class: "selo-area" });
  caixa.append(area,
    el("div", { class: "final-acoes" },
      el("a", { class: "btn primary grande", href: "/#baixar" }, (() => { const s = svgEl("svg", { class: "ico" }); s.append(svgEl("use", { href: "#i-down" })); return s; })(), el("span", {}, "Baixar o programa e operar")),
      el("a", { class: "btn", href: "/#site-t" }, "Acompanhar clientes de longe (opcional)")),
    el("p", { class: "tiny" }, "Lembre: aqui é aprendizagem e validação. No cliente, a solução definitiva é a câmera ou o gravador com IA de borda, ou uma plataforma parceira. O CamFácil IA é a ponte."));
  if (!tudo) {
    area.append(el("div", { class: "selo" }, (() => { const s = svgEl("svg", { class: "ico" }); s.append(svgEl("use", { href: "#i-selo" })); return s; })(),
      el("div", {}, el("strong", {}, "Selo ainda não liberado"), el("p", { class: "tiny" }, "Volte e faça os exercícios das lições que faltam."))));
    return caixa;
  }
  montarSelo(area);
  return caixa;
}

async function montarSelo(area) {
  area.replaceChildren(el("p", { class: "tiny" }, "Conferindo o seu selo…"));
  let sessao = null;
  try { const r = await fetch("/api/escola/selo", { credentials: "same-origin" }); if (r.ok) sessao = await r.json(); } catch (_) { /* offline */ }
  if (!sessao) {
    // Sem login: explica e manda para o Google, voltando para cá.
    area.replaceChildren(el("div", { class: "selo ganho" },
      (() => { const s = svgEl("svg", { class: "ico" }); s.append(svgEl("use", { href: "#i-selo" })); return s; })(),
      el("div", {},
        el("strong", {}, "Você concluiu a formação. Falta só gerar o seu selo."),
        el("p", { class: "tiny" }, "O selo leva o seu nome e a data, e fica numa página que você pode compartilhar no Instagram, no LinkedIn e no WhatsApp. Para isso, entre com a sua conta Google (sem senha nova)."),
        el("a", { class: "btn primary", href: `/api/auth/google?voltar=${encodeURIComponent("/aprender#pratica")}` }, "Entrar com Google e gerar o selo"))));
    return;
  }
  if (sessao.selo) { area.replaceChildren(window.cartaoSelo(sessao.selo, { compacto: true })); falar("escola-selo"); return; }
  const inst = el("input", { name: "instagram", maxlength: "31", placeholder: "seu.usuario", autocapitalize: "off", spellcheck: "false" });
  const autoriza = el("input", { type: "checkbox", name: "autoriza" });
  const btn = el("button", { class: "btn primary", type: "submit" }, "Gerar meu selo");
  const msg = el("p", { class: "msg" });
  const form = el("form", { class: "form selo-form", autocomplete: "off" },
    el("p", { class: "kicker span-4" }, `Olá, ${sessao.nome || "técnico"}. Último passo.`),
    el("label", { class: "span-2" }, el("span", {}, "Seu Instagram (opcional)"), el("span", { class: "prefixo" }, el("i", {}, "@"), inst)),
    el("label", { class: "span-4 check" }, autoriza, el("span", {}, "Autorizo o CamFácil IA (Emerson), o Papo Segurança e a Zion a repostar o meu selo nos stories, marcando o meu Instagram.")),
    el("div", { class: "actions span-4" }, btn),
    msg);
  form.onsubmit = async (ev) => {
    ev.preventDefault(); btn.disabled = true; msg.textContent = ""; msg.className = "msg";
    try {
      const r = await fetch("/api/escola/selo", { method: "POST", credentials: "same-origin", headers: { "content-type": "application/json" }, body: JSON.stringify({ instagram: inst.value || null, autoriza_repost: autoriza.checked }) });
      const j = await r.json();
      if (!r.ok) throw new Error(j.erro || "não deu para gerar o selo");
      area.replaceChildren(window.cartaoSelo(j.selo, { compacto: true }));
      falar("escola-selo");
    } catch (e) { msg.className = "msg err"; msg.textContent = e.message; btn.disabled = false; }
  };
  area.replaceChildren(form);
}

async function main() {
  temNarracao = await narracaoDisponivel();
  $("#ouvir").onclick = () => { const l = LICOES[atual]; if (!voz.paused && !voz.ended) voz.pause(); else tocar(l.audio, true); };
  $("#voltar").onclick = () => ir(atual - 1);
  $("#proxima").onclick = () => ir(atual + 1);
  const hash = location.hash.replace("#", "");
  const porHash = LICOES.findIndex((l) => l.id === hash || (hash === "cerca" && l.id === "linha"));
  const p = progresso();
  ir(porHash >= 0 ? porHash : (p.atual || 0));
}
main();

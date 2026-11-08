// Mosaico ao vivo, leve, para abrir pelo celular (Tailscale) ou pelo app Android.
// Só a imagem e o nome da câmera: nada de caixas da IA.
// Zero rolagem: as câmeras dividem a tela; o que não cabe vai para a próxima página.
(async () => {
  const $ = (id) => document.getElementById(id);
  const m = $("mosaico");
  const so = new URLSearchParams(location.search).get("camera");
  let T = {};
  try { T = await (await fetch("/i18n/pt-BR.json")).json(); } catch (_) { /* textos padrão */ }
  const t = (k, padrao) => T[k] ?? padrao;
  let cams = [];
  try { cams = (await (await fetch("/api/cameras")).json()).cameras || []; } catch (_) { /* sem conexão */ }
  cams = cams.filter((c) => c.ativa !== false && (!so || String(c.id) === so));
  if (!cams.length) {
    m.innerHTML = '<p class="vazio">Nenhuma câmera ligada neste computador ainda. Ligue uma no painel completo.</p>';
    return;
  }
  $("ant").setAttribute("aria-label", t("pag.anterior", "Página anterior"));
  $("prox").setAttribute("aria-label", t("pag.proxima", "Próxima página"));
  let pag = 0;

  // Quantas por página: 4 no celular em pé, 9 em telas maiores.
  const porPagina = () => (innerWidth < 600 ? 4 : 9);

  // Escolhe colunas x linhas que deixam cada imagem 16:9 o maior possível.
  function grade(n) {
    const W = m.clientWidth, H = m.clientHeight;
    let melhor = { c: 1, r: n, lado: 0 };
    for (let c = 1; c <= n; c++) {
      const r = Math.ceil(n / c);
      const lado = Math.min(W / c, ((H / r) * 16) / 9);
      if (lado > melhor.lado) melhor = { c, r, lado };
    }
    m.style.gridTemplateColumns = `repeat(${melhor.c}, minmax(0, 1fr))`;
    m.style.gridTemplateRows = `repeat(${melhor.r}, minmax(0, 1fr))`;
  }

  function mostrar() {
    const n = porPagina();
    const paginas = Math.ceil(cams.length / n);
    pag = Math.max(0, Math.min(pag, paginas - 1));
    const vis = cams.slice(pag * n, pag * n + n);
    $("pag").hidden = paginas < 2;
    $("ant").disabled = pag === 0;
    $("prox").disabled = pag >= paginas - 1;
    const total = `${cams.length} ${cams.length === 1 ? "câmera" : "câmeras"}`;
    $("info").textContent = paginas > 1 ? `${total} · ${pag + 1} ${t("pag.de", "de")} ${paginas}` : total;
    // Só as câmeras desta página puxam vídeo (economiza a rede do celular).
    m.replaceChildren(...vis.map((c) => {
      const f = document.createElement("figure");
      const img = document.createElement("img");
      img.alt = c.nome || "Câmera";
      img.src = `/api/cameras/${c.id}/mjpeg?t=${Date.now()}`;
      img.onerror = () => { f.classList.add("off"); setTimeout(() => { if (img.isConnected) { img.src = `/api/cameras/${c.id}/mjpeg?t=${Date.now()}`; f.classList.remove("off"); } }, 5000); };
      const cap = document.createElement("figcaption");
      cap.append(document.createElement("i"), document.createTextNode(c.nome || "Câmera"));
      f.append(img, cap);
      f.onclick = () => f.classList.toggle("cheia");
      return f;
    }));
    grade(vis.length);
    if (cams.length === 1) m.firstChild.classList.add("cheia");
  }
  $("ant").onclick = () => { pag--; mostrar(); };
  $("prox").onclick = () => { pag++; mostrar(); };
  let n0 = porPagina();
  addEventListener("resize", () => {
    if (porPagina() !== n0) { n0 = porPagina(); mostrar(); } else grade(m.children.length);
  });
  mostrar();
})();

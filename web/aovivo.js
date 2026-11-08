// Mosaico ao vivo, leve, para abrir pelo celular (Tailscale) ou pelo app Android.
// Só a imagem e o nome da câmera: nada de caixas da IA.
(async () => {
  const m = document.getElementById("mosaico");
  const so = new URLSearchParams(location.search).get("camera");
  let cams = [];
  try { cams = (await (await fetch("/api/cameras")).json()).cameras || []; } catch (_) { /* sem conexão */ }
  cams = cams.filter((c) => c.ativa !== false && (!so || String(c.id) === so));
  document.getElementById("info").textContent = cams.length ? `${cams.length} ${cams.length === 1 ? "câmera" : "câmeras"}` : "";
  if (!cams.length) {
    m.innerHTML = '<p class="vazio">Nenhuma câmera ligada neste computador ainda. Ligue uma no painel completo.</p>';
    return;
  }
  for (const c of cams) {
    const f = document.createElement("figure");
    const img = document.createElement("img");
    img.alt = c.nome || "Câmera";
    img.src = `/api/cameras/${c.id}/mjpeg?t=${Date.now()}`;
    img.onerror = () => { f.classList.add("off"); setTimeout(() => { img.src = `/api/cameras/${c.id}/mjpeg?t=${Date.now()}`; f.classList.remove("off"); }, 5000); };
    const cap = document.createElement("figcaption");
    cap.append(document.createElement("i"), document.createTextNode(c.nome || "Câmera"));
    f.append(img, cap);
    f.onclick = () => f.classList.toggle("cheia");
    m.append(f);
  }
  if (cams.length === 1) m.firstChild.classList.add("cheia");
})();

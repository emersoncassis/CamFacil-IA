// Manifesto narrado na voz do Emerson: uma fala por seção; a página vai
// para a seção que está sendo lida e avança sozinha. JS puro (CSP).
"use strict";
(() => {
  const caixa = document.querySelector("[data-paginas]");
  const botao = document.getElementById("ouvir-manifesto");
  if (!caixa || !botao) return;
  const rotulo = botao.querySelector("span");
  const uso = botao.querySelector("use");
  const partes = ["/audio/manifesto-01.ogg", "/audio/manifesto-02.ogg", "/audio/manifesto-03.ogg", "/audio/manifesto-04.ogg"];
  const voz = new Audio();
  voz.preload = "none";
  let parte = 0;
  const estado = (falando) => {
    botao.classList.toggle("falando", falando);
    rotulo.textContent = falando ? "Pausar" : (parte > 0 && parte < partes.length ? "Continuar" : "Ouvir o manifesto");
    uso.setAttribute("href", falando ? "#i-pause" : "#i-play");
  };
  const tocar = (i) => {
    parte = i;
    caixa.dispatchEvent(new CustomEvent("doc-ir", { detail: i }));
    voz.src = partes[i];
    voz.play().catch(() => estado(false));
  };
  voz.onplaying = () => estado(true);
  voz.onpause = () => estado(false);
  voz.onended = () => { if (parte + 1 < partes.length) tocar(parte + 1); else { parte = 0; estado(false); } };
  botao.onclick = () => {
    if (!voz.paused) { voz.pause(); return; }
    if (voz.src && parte < partes.length && voz.currentTime > 0 && !voz.ended) voz.play();
    else tocar(parte);
  };
  estado(false);
})();

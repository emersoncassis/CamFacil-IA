// Manifesto narrado na página inicial: toca as 4 partes em sequência
// e abre a seção que está sendo lida. JS puro, sem dependências.
(function () {
  var script = document.currentScript || document.querySelector('script[src*="manifesto.js"]');
  var base = new URL("../audio/", script.src).href;
  var partes = [1, 2, 3, 4].map(function (n) { return base + "manifesto-0" + n + ".mp3"; });
  var duracoes = [40.6, 41.0, 23.4, 21.9];
  var total = duracoes.reduce(function (a, b) { return a + b; }, 0);
  var audio = null, atual = 0;

  function montar() {
    var player = document.querySelector(".player");
    document.body.classList.toggle("cf-home", !!document.querySelector(".manifesto"));
    if (!player || player.dataset.pronto) return;
    player.dataset.pronto = "1";

    var btn = player.querySelector(".player__btn");
    var barra = player.querySelector(".player__bar");
    var preenchido = barra.querySelector("span");
    var tempo = player.querySelector(".player__tempo");
    var secoes = Array.prototype.slice.call(document.querySelectorAll("details.parte"));

    if (audio) { audio.pause(); }
    audio = new Audio();
    audio.preload = "none";
    atual = 0;

    function fmt(s) { s = Math.max(0, Math.round(s)); return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0"); }
    function decorrido() {
      var t = 0; for (var i = 0; i < atual; i++) t += duracoes[i];
      return t + (audio.currentTime || 0);
    }
    function destacar(i) {
      secoes.forEach(function (d, j) {
        d.classList.toggle("ao-vivo", j === i);
        if (j === i) d.open = true; else if (i >= 0) d.open = false;
      });
    }
    function tocar(i, desde) {
      atual = i;
      audio.src = partes[i];
      audio.currentTime = desde || 0;
      destacar(i);
      audio.play().catch(function () { player.classList.remove("tocando"); });
      player.classList.add("tocando");
    }

    audio.addEventListener("timeupdate", function () {
      var d = decorrido();
      preenchido.style.width = (100 * d / total) + "%";
      if (tempo) tempo.textContent = fmt(d) + " / " + fmt(total);
    });
    audio.addEventListener("ended", function () {
      if (atual < partes.length - 1) { tocar(atual + 1); return; }
      player.classList.remove("tocando");
      destacar(-1);
      atual = 0;
    });

    btn.addEventListener("click", function () {
      if (player.classList.contains("tocando")) {
        audio.pause(); player.classList.remove("tocando");
      } else if (audio.src && audio.currentTime > 0) {
        audio.play(); player.classList.add("tocando");
      } else {
        tocar(atual);
      }
    });

    barra.addEventListener("click", function (e) {
      var alvo = total * (e.offsetX / barra.clientWidth), acc = 0;
      for (var i = 0; i < duracoes.length; i++) {
        if (alvo < acc + duracoes[i]) { tocar(i, alvo - acc); return; }
        acc += duracoes[i];
      }
    });

    // No celular, tudo começa fechado para caber numa tela só
    if (window.matchMedia("(max-width: 960px)").matches) secoes.forEach(function (d) { d.open = false; });

    // Abrir uma parte deixa as outras fechadas: a página nunca cresce
    secoes.forEach(function (d) {
      d.addEventListener("toggle", function () {
        if (d.open) secoes.forEach(function (o) { if (o !== d) o.open = false; });
      });
    });
  }

  if (window.document$ && window.document$.subscribe) {
    window.document$.subscribe(montar);
  } else {
    document.addEventListener("DOMContentLoaded", montar);
  }
})();

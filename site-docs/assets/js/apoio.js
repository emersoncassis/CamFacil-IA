// Página de apoiadores: copiar o Pix e enviar o formulário do mural.
// O envio cai numa planilha e numa pasta privadas do Google Drive do autor
// (Apps Script em deploy/apoio/Codigo.gs). Nada fica público sem ele aprovar.
(function () {
  var EMAIL = "emersonassis.work@gmail.com";
  // URL /exec do Apps Script (deploy/apoio/Codigo.gs). Vazio: o botão abre o e-mail.
  var APOIO_URL = "";

  function montar() {
    document.querySelectorAll("[data-copiar]").forEach(function (b) {
      if (b.dataset.pronto) return;
      b.dataset.pronto = "1";
      b.addEventListener("click", function () {
        var texto = b.getAttribute("data-copiar");
        var ok = function () {
          var antes = b.textContent;
          b.textContent = b.getAttribute("data-copiado") || "Copiado";
          setTimeout(function () { b.textContent = antes; }, 1800);
        };
        if (navigator.clipboard) navigator.clipboard.writeText(texto).then(ok, ok);
        else { var t = document.createElement("textarea"); t.value = texto; document.body.appendChild(t); t.select(); try { document.execCommand("copy"); } catch (e) {} t.remove(); ok(); }
      });
    });

    var form = document.querySelector("form.apoio-form");
    if (!form || form.dataset.pronto) return;
    form.dataset.pronto = "1";
    var aviso = form.querySelector(".apoio-aviso");
    var botao = form.querySelector("[data-enviar]");

    function avisar(chave, tipo) {
      aviso.textContent = form.getAttribute("data-msg-" + chave) || "";
      aviso.className = "apoio-aviso " + (tipo || "");
    }
    function campo(n) { var el = form.elements[n]; return el ? (el.type === "checkbox" ? el.checked : el.value.trim()) : ""; }
    function lerArquivo(f) {
      return new Promise(function (ok, erro) {
        if (!f) return ok(null);
        if (f.size > 6 * 1024 * 1024) return erro("grande");
        var r = new FileReader();
        r.onload = function () { ok({ nome: f.name, tipo: f.type, dados: String(r.result).split(",")[1] }); };
        r.onerror = function () { erro("arquivo"); };
        r.readAsDataURL(f);
      });
    }
    function porEmail() {
      var linhas = [form.getAttribute("data-titulo"), ""];
      form.querySelectorAll("[data-campo]").forEach(function (el) {
        var v = el.type === "checkbox" ? (el.checked ? el.getAttribute("data-sim") : el.getAttribute("data-nao")) : el.value.trim();
        if (v) linhas.push(el.getAttribute("data-campo") + ": " + v);
      });
      linhas.push("", form.getAttribute("data-anexo"));
      location.href = "mailto:" + EMAIL + "?subject=" + encodeURIComponent(form.getAttribute("data-assunto")) + "&body=" + encodeURIComponent(linhas.join("\n"));
    }

    form.addEventListener("submit", function (e) {
      e.preventDefault();
      if (!form.reportValidity()) return;
      if (!APOIO_URL) { porEmail(); return; }
      botao.disabled = true;
      avisar("enviando");
      var arquivo = form.elements.comprovante && form.elements.comprovante.files[0];
      lerArquivo(arquivo).then(function (arq) {
        var dados = {
          nome: campo("nome"), link: campo("link"), valor: campo("valor"), tipo: campo("tipo"),
          recado: campo("recado"), quer: campo("pedido"), publicar: campo("publicar"),
          site: campo("site"), idioma: document.documentElement.lang || "pt", arquivo: arq
        };
        // text/plain evita a checagem prévia de CORS; o Apps Script lê o corpo cru
        return fetch(APOIO_URL, { method: "POST", mode: "no-cors", headers: { "Content-Type": "text/plain;charset=utf-8" }, body: JSON.stringify(dados) });
      }).then(function () {
        form.reset();
        avisar("ok", "ok");
      }).catch(function (motivo) {
        avisar(motivo === "grande" ? "grande" : "erro", "erro");
      }).then(function () { botao.disabled = false; });
    });
  }

  if (window.document$ && window.document$.subscribe) window.document$.subscribe(montar);
  else document.addEventListener("DOMContentLoaded", montar);
})();

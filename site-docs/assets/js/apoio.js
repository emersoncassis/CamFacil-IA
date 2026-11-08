// Página de apoiadores: copiar o Pix e montar a mensagem para entrar no mural.
// Nada é enviado a servidor nenhum: o botão abre o e-mail ou o WhatsApp da
// própria pessoa, com o texto pronto, e ela anexa o comprovante.
(function () {
  var EMAIL = "emersonassis.work@gmail.com";
  var WHATSAPP = ""; // número com DDI e DDD, só dígitos (ex.: 5517999999999). Vazio esconde o botão.

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

    var zap = form.querySelector("[data-canal=whatsapp]");
    if (zap && !WHATSAPP) zap.hidden = true;

    function mensagem() {
      var linhas = [form.getAttribute("data-titulo") || "", ""];
      form.querySelectorAll("[data-campo]").forEach(function (el) {
        var rotulo = el.getAttribute("data-campo");
        var valor = el.type === "checkbox" ? (el.checked ? el.getAttribute("data-sim") : el.getAttribute("data-nao")) : el.value.trim();
        if (valor) linhas.push(rotulo + ": " + valor);
      });
      linhas.push("", form.getAttribute("data-anexo") || "");
      return linhas.join("\n");
    }

    form.addEventListener("submit", function (e) { e.preventDefault(); });
    form.querySelectorAll("[data-canal]").forEach(function (b) {
      b.addEventListener("click", function () {
        if (!form.reportValidity()) return;
        var texto = mensagem();
        if (b.getAttribute("data-canal") === "email") {
          location.href = "mailto:" + EMAIL + "?subject=" + encodeURIComponent(form.getAttribute("data-assunto") || "CamFácil IA") + "&body=" + encodeURIComponent(texto);
        } else {
          window.open("https://wa.me/" + WHATSAPP + "?text=" + encodeURIComponent(texto), "_blank", "noopener");
        }
      });
    });
  }

  if (window.document$ && window.document$.subscribe) window.document$.subscribe(montar);
  else document.addEventListener("DOMContentLoaded", montar);
})();

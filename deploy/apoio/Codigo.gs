/**
 * Recebe o formulário "Entrar no mural" do site do CamFácil IA.
 * Cada envio vira uma linha na planilha "CamFácil IA — Apoiadores"
 * e o comprovante fica na pasta "CamFácil IA — Comprovantes" do Drive,
 * os dois privados, só do dono. Também manda um aviso por e-mail.
 *
 * Como publicar (uma vez só):
 *  1. Abra a planilha → Extensões → Apps Script, apague o que tiver e cole este arquivo.
 *  2. Implantar → Nova implantação → tipo "App da Web".
 *     Executar como: Eu · Quem pode acessar: Qualquer pessoa → Implantar → Autorizar.
 *  3. Copie a URL que termina em /exec e coloque em APOIO_URL, no site-docs/assets/js/apoio.js.
 */
var PLANILHA_ID = "1ktU80Au3qxjoBXeCNGUd3-v6gEAwb5AazPOEt9HHw3k";
var PASTA_ID = "14Q0SER03lSLxZy-0wCwv4XZyA_HyUhZa";
var AVISAR = true; // e-mail para o dono a cada apoio novo
var LIMITE_ARQUIVO = 6 * 1024 * 1024; // 6 MB
var TIPOS_ARQUIVO = ["image/jpeg", "image/png", "image/webp", "image/heic", "application/pdf"];

function doPost(e) {
  var lock = LockService.getScriptLock();
  lock.waitLock(20000);
  try {
    var d = JSON.parse(e.postData.contents || "{}");
    if (d.site) return resposta({ ok: true }); // campo-isca: robôs preenchem, pessoas não veem

    var nome = corta(d.nome, 60);
    if (!nome) return resposta({ ok: false, erro: "nome" });

    var linkComprovante = "";
    if (d.arquivo && d.arquivo.dados) {
      var tipo = String(d.arquivo.tipo || "");
      var bytes = Utilities.base64Decode(d.arquivo.dados);
      if (TIPOS_ARQUIVO.indexOf(tipo) >= 0 && bytes.length <= LIMITE_ARQUIVO) {
        var nomeArq = Utilities.formatDate(new Date(), "America/Sao_Paulo", "yyyy-MM-dd HH-mm") + " " + nome + " " + corta(d.arquivo.nome, 60);
        var arq = DriveApp.getFolderById(PASTA_ID).createFile(Utilities.newBlob(bytes, tipo, nomeArq));
        linkComprovante = arq.getUrl();
      }
    }

    var linha = [
      new Date(), nome, corta(d.link, 120), corta(d.valor, 20), corta(d.tipo, 40),
      corta(d.recado, 400), corta(d.quer, 600), d.publicar ? "sim" : "não",
      corta(d.idioma, 5), linkComprovante, "novo"
    ];
    SpreadsheetApp.openById(PLANILHA_ID).getSheets()[0].appendRow(linha.map(semFormula));

    if (AVISAR) {
      MailApp.sendEmail(Session.getEffectiveUser().getEmail(),
        "Novo apoio ao CamFácil IA: " + nome,
        "Nome: " + nome + "\nValor: " + linha[3] + "\nTipo: " + linha[4] +
        "\nRecado: " + linha[5] + "\nO que quer: " + linha[6] +
        "\nPublicar no mural: " + linha[7] + "\nComprovante: " + (linkComprovante || "não enviado") +
        "\n\nPlanilha: https://docs.google.com/spreadsheets/d/" + PLANILHA_ID);
    }
    return resposta({ ok: true });
  } catch (err) {
    return resposta({ ok: false, erro: String(err) });
  } finally {
    lock.releaseLock();
  }
}

function doGet() { return resposta({ ok: true, servico: "apoio CamFácil IA" }); }

function corta(v, n) { return String(v == null ? "" : v).trim().slice(0, n); }
// Impede que um texto vire fórmula na planilha
function semFormula(v) { return (typeof v === "string" && /^[=+\-@]/.test(v)) ? "'" + v : v; }
function resposta(o) { return ContentService.createTextOutput(JSON.stringify(o)).setMimeType(ContentService.MimeType.JSON); }

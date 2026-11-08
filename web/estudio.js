// CamFácil IA — modo estúdio no programa local (aba Módulos): coletar fotos de
// uma câmera numa pasta local e importar, ligar ou apagar o modelo de estudo
// treinado no Colab. Usa api, el, t, $, confirmar e ieMsg dos outros scripts.
// JS puro.
"use strict";

let esTimer = null;

async function esCameras() {
  const sel = $("#es-coleta-form").camera;
  let cams = [];
  try { ({ cameras: cams } = await api("/api/cameras")); } catch (_) { return; }
  const atual = sel.value;
  sel.replaceChildren(...(cams.length
    ? cams.map((c) => el("option", { value: String(c.id) }, c.nome))
    : [el("option", { value: "" }, t("es.sem_camera"))]));
  if (atual) sel.value = atual;
}

async function esColeta() {
  let e;
  try { e = await api("/api/estudio/coleta"); } catch (_) { return; }
  const f = $("#es-coleta-form");
  $("#es-coletar").hidden = e.ativa;
  $("#es-parar").hidden = !e.ativa;
  $("#es-apagar-fotos").hidden = e.ativa || !e.receita || !e.fotos_na_pasta;
  $("#es-apagar-fotos").dataset.receita = e.receita || "";
  [...f.elements].forEach((x) => { if (x.tagName !== "BUTTON") x.disabled = e.ativa; });
  const txt = !e.receita ? t("es.coleta_nenhuma")
    : t(e.ativa ? "es.coleta_andando" : "es.coleta_fim")
      .replace("{n}", e.salvas).replace("{total}", e.total)
      .replace("{pasta}", e.pasta).replace("{na_pasta}", e.fotos_na_pasta);
  $("#es-coleta-estado").textContent = txt;
  if (e.erro) ieMsg("#es-coleta-msg", e.erro, "err");
}

async function esModelos() {
  let r;
  try { r = await api("/api/estudio/modelos"); } catch (e) { ieMsg("#es-msg", e.message, "err"); return; }
  const corpo = $("#es-modelos");
  if (!r.modelos.length) {
    corpo.replaceChildren(el("tr", { class: "empty" }, el("td", { colspan: "3" }, t("es.nenhum_modelo"))));
    return;
  }
  corpo.replaceChildren(...r.modelos.map((m) => {
    const pronto = m.situacao === "pronto";
    const input = el("input", { type: "checkbox", "aria-label": t("es.ligar_dica").replace("{nome}", m.nome) });
    input.checked = !!m.ligado;
    if (!pronto) input.disabled = true;
    input.onchange = async () => {
      const ligar = input.checked;
      if (ligar && !await confirmar(t("es.confirma_ligar"), t("es.ligar"))) { input.checked = false; return; }
      try {
        await api(`/api/estudio/modelos/${m.id}/ligar`, { ligado: ligar });
        ieMsg("#es-msg", t(ligar ? "es.ligado_msg" : "es.desligado_msg"), "ok");
      } catch (e) { ieMsg("#es-msg", e.message, "err"); }
      esModelos();
    };
    const apagar = el("button", { class: "btn ghost sm", type: "button" }, t("es.apagar"));
    apagar.onclick = async () => {
      if (!await confirmar(t("es.confirma_apagar").replace("{nome}", m.nome))) return;
      try { await api(`/api/estudio/modelos/${m.id}`, undefined, "DELETE"); } catch (e) { ieMsg("#es-msg", e.message, "err"); }
      esModelos();
    };
    const situacao = pronto ? t(m.ligado ? "es.situacao_ligado" : "es.situacao_pronto") : t("es.situacao_falta_onnx");
    return el("tr", {},
      el("td", {}, m.nome, el("span", { class: "tiny" }, ` · ${t("es.estudo")} · ${situacao}`)),
      el("td", {}, m.classes.join(", ")),
      el("td", { class: "right" }, el("div", { class: "actions end" }, apagar, el("label", { class: "switch" }, input, el("span")))));
  }));
}

const esLer = (arquivo) => arquivo.text().then((s) => JSON.parse(s)).catch(() => { throw new Error(t("es.json_invalido").replace("{arq}", arquivo.name)); });

async function esImportar(ev) {
  ev.preventDefault();
  const f = ev.target;
  try {
    const receita = await esLer(f.receita.files[0]);
    const manifesto = f.manifesto.files[0] ? await esLer(f.manifesto.files[0]) : null;
    const r = await api("/api/estudio/modelos", { receita, manifesto });
    const res = await fetch(r.proximo, { method: "PUT", body: f.onnx.files[0] });
    const d = await res.json().catch(() => ({}));
    if (!res.ok) {
      await api(`/api/estudio/modelos/${r.modelo.id}`, undefined, "DELETE").catch(() => {});
      throw new Error(d.erro || res.statusText);
    }
    f.reset();
    ieMsg("#es-msg", t("es.importado"), "ok");
  } catch (e) { ieMsg("#es-msg", e.message, "err"); }
  esModelos();
}

function esAtivar(ativo) {
  clearInterval(esTimer); esTimer = null;
  if (!ativo) return;
  esCameras(); esColeta(); esModelos();
  esTimer = setInterval(esColeta, 3000);
}

function iniciarEstudioLocal() {
  const f = $("#es-coleta-form");
  if (!f) return;
  f.onsubmit = async (ev) => {
    ev.preventDefault();
    try {
      await api("/api/estudio/coleta", {
        camera_id: Number(f.camera.value), receita: f.receita.value.trim(),
        intervalo_s: Number(f.intervalo.value), total: Number(f.total.value),
      });
      ieMsg("#es-coleta-msg", t("es.coleta_comecou"), "ok");
    } catch (e) { ieMsg("#es-coleta-msg", e.message, "err"); }
    esColeta();
  };
  $("#es-parar").onclick = async () => { await api("/api/estudio/coleta/parar", {}).catch(() => {}); esColeta(); };
  $("#es-importar-form").onsubmit = esImportar;
  $("#es-apagar-fotos").onclick = async (ev) => {
    const receita = ev.currentTarget.dataset.receita;
    if (!await confirmar(t("es.confirma_apagar_fotos").replace("{receita}", receita))) return;
    try { await api(`/api/estudio/coleta/${receita}`, undefined, "DELETE"); ieMsg("#es-coleta-msg", t("es.fotos_apagadas"), "ok"); }
    catch (e) { ieMsg("#es-coleta-msg", e.message, "err"); }
    esColeta();
  };
  const painel = $("#modulos");
  new MutationObserver(() => esAtivar(!painel.hidden)).observe(painel, { attributes: true, attributeFilter: ["hidden"] });
  esAtivar(!painel.hidden);
}


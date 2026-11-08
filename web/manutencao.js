// CamFácil IA — tela Manutenção (fase 5). Usa os ajudantes do app.js
// (t, $, el, api), carregado logo depois dele.
"use strict";

function manMsg(id, texto, tipo) { const m = $(id); m.textContent = texto || ""; m.className = "msg " + (tipo || ""); }

// ---------- Modo TV ----------
let tvInfo = null;
let tvOrdem = []; // ids na ordem do mosaico

function tvRenderCameras() {
  const lista = $("#tv-cameras");
  if (!tvInfo || !tvInfo.cameras.length) {
    lista.replaceChildren(el("li", {}, el("p", {}, t("tv.sem_cameras_cfg"))));
    return;
  }
  const todas = tvInfo.cameras;
  // Ordem: as escolhidas primeiro, depois as não escolhidas.
  const ordenadas = [...tvOrdem.map((id) => todas.find((c) => c.id === id)).filter(Boolean), ...todas.filter((c) => !tvOrdem.includes(c.id))];
  lista.replaceChildren(...ordenadas.map((c, i) => {
    const marcada = tvOrdem.includes(c.id);
    const chk = el("input", { type: "checkbox", "aria-label": c.nome });
    chk.checked = marcada;
    chk.onchange = () => { if (chk.checked) tvOrdem.push(c.id); else tvOrdem = tvOrdem.filter((x) => x !== c.id); tvRenderCameras(); };
    const sobe = el("button", { class: "btn ghost sm", type: "button", title: t("tv.subir"), disabled: "" }, "↑");
    const desce = el("button", { class: "btn ghost sm", type: "button", title: t("tv.descer"), disabled: "" }, "↓");
    if (marcada) {
      const pos = tvOrdem.indexOf(c.id);
      if (pos > 0) { sobe.removeAttribute("disabled"); sobe.onclick = () => { [tvOrdem[pos - 1], tvOrdem[pos]] = [tvOrdem[pos], tvOrdem[pos - 1]]; tvRenderCameras(); }; }
      if (pos < tvOrdem.length - 1) { desce.removeAttribute("disabled"); desce.onclick = () => { [tvOrdem[pos + 1], tvOrdem[pos]] = [tvOrdem[pos], tvOrdem[pos + 1]]; tvRenderCameras(); }; }
    }
    return el("li", {},
      el("div", {}, el("label", { class: "som-regra" }, el("span", { class: "switch" }, chk, el("span")), el("strong", {}, c.nome),
        el("span", { class: "tiny" }, marcada ? `${t("tv.posicao")} ${i + 1}` : t("tv.fora_do_mosaico"))),
        el("p", {}, `${t("fonte." + c.fonte)} · ${c.online ? t("estado.online") : t("estado.conectando")}`)),
      el("span", { class: "ordem" }, sobe, desce));
  }));
}

function tvEndereco() {
  if (!tvInfo) return "";
  const ip = tvInfo.lan && tvInfo.ip_rede ? tvInfo.ip_rede : location.hostname;
  return `http://${ip}:${tvInfo.porta}/tv`;
}

async function tvCarregar() {
  try {
    tvInfo = await api("/api/tv");
    const c = tvInfo.config;
    tvOrdem = c.cameras.length ? c.cameras.filter((id) => tvInfo.cameras.some((k) => k.id === id)) : tvInfo.cameras.map((k) => k.id);
    tvRenderCameras();
    $("#tv-whatsapp").value = c.whatsapp || "";
    $("#tv-chamada").value = c.chamada || "";
    $("#tv-som").checked = !!c.som;
    $("#tv-piscar").checked = !!c.piscar;
    $("#tv-relogio").checked = !!c.relogio;
    $("#tv-endereco").textContent = tvEndereco();
    $("#tv-endereco-dica").textContent = tvInfo.lan ? t("tv.endereco_lan") : t("tv.endereco_local");
    tvLogoRender();
    $("#tv-passos").replaceChildren(...(T["tv.passos"] || []).map((p) => el("li", {}, p.replace("{url}", tvEndereco()))));
  } catch (e) { manMsg("#tv-msg", e.message, "err"); }
}

function tvLogoRender() {
  const img = $("#tv-logo-img");
  const tem = tvInfo && tvInfo.logo;
  img.hidden = !tem; $("#tv-logo-vazio").hidden = tem; $("#tv-logo-remover").hidden = !tem;
  if (tem) img.src = "/api/tv/logo.png?" + Date.now();
}

async function tvSalvar() {
  const b = $("#tv-salvar"); b.disabled = true;
  try {
    tvInfo = await api("/api/tv", {
      cameras: tvOrdem, som: $("#tv-som").checked, piscar: $("#tv-piscar").checked, relogio: $("#tv-relogio").checked,
      whatsapp: $("#tv-whatsapp").value, chamada: $("#tv-chamada").value,
    });
    $("#tv-whatsapp").value = tvInfo.config.whatsapp;
    manMsg("#tv-msg", tvInfo.whatsapp_link ? `${t("tv.salvo")} · ${tvInfo.whatsapp_link}` : t("tv.salvo"), "ok");
  } catch (e) { manMsg("#tv-msg", e.message, "err"); }
  b.disabled = false;
}

async function tvEnviarLogo(file) {
  if (!file) return;
  if (file.size > 200 * 1024) { manMsg("#tv-msg", t("tv.logo_grande"), "err"); return; }
  try {
    const r = await fetch("/api/tv/logo", { method: "POST", headers: { "Content-Type": "image/png" }, body: file });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.erro || r.statusText);
    tvInfo.logo = true; tvLogoRender(); manMsg("#tv-msg", t("tv.logo_ok"), "ok");
  } catch (e) { manMsg("#tv-msg", e.message, "err"); }
}

function tvIniciar() {
  $("#tv-salvar").onclick = tvSalvar;
  $("#tv-copiar").onclick = async () => { try { await navigator.clipboard.writeText(tvEndereco()); manMsg("#tv-msg", t("tv.copiado"), "ok"); } catch (_) { /* sem permissão */ } };
  $("#tv-logo-file").onchange = (e) => { tvEnviarLogo(e.target.files[0]); e.target.value = ""; };
  $("#tv-logo-remover").onclick = async () => { try { await api("/api/tv/logo", undefined, "DELETE"); tvInfo.logo = false; tvLogoRender(); } catch (e) { manMsg("#tv-msg", e.message, "err"); } };
}

// ---------- Entrada da tela ----------
let manutencaoCarregada = false;
function manutencaoAtivar(ativo) {
  if (!ativo) return;
  tvCarregar();
  manutencaoCarregada = true;
}

function iniciarManutencao() {
  tvIniciar();
}

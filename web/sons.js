// Sons do alerta (gerados na hora, sem arquivo de áudio). Cada regra escolhe um.
// Uso: CFSons.tocar(audioContext, "sino").
(function () {
  function nota(c, t0, freq, tipo, dur, pico, ate) {
    const o = c.createOscillator();
    const g = c.createGain();
    o.type = tipo;
    o.frequency.setValueAtTime(freq, t0);
    if (ate) o.frequency.exponentialRampToValueAtTime(ate, t0 + dur);
    g.gain.setValueAtTime(0.0001, t0);
    g.gain.exponentialRampToValueAtTime(pico, t0 + 0.02);
    g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
    o.connect(g).connect(c.destination);
    o.start(t0);
    o.stop(t0 + dur + 0.02);
  }
  const SONS = {
    // Dois bipes curtos (o padrão de sempre).
    bipe: (c, t) => { nota(c, t, 880, "sine", 0.16, 0.25); nota(c, t + 0.18, 1320, "sine", 0.16, 0.25); },
    // Sino: um tom com harmônicos que demora a sumir.
    sino: (c, t) => { nota(c, t, 660, "sine", 1.0, 0.22); nota(c, t, 1320, "sine", 0.7, 0.1); nota(c, t, 1980, "sine", 0.4, 0.05); },
    // Alarme: três pares de tons alternados, mais firme.
    alarme: (c, t) => { for (let i = 0; i < 3; i++) { nota(c, t + i * 0.3, 740, "square", 0.13, 0.1); nota(c, t + i * 0.3 + 0.14, 988, "square", 0.13, 0.1); } },
    // Gota: um toque que cai de agudo para grave.
    gota: (c, t) => nota(c, t, 1400, "sine", 0.28, 0.3, 420),
    silencio: () => {},
  };
  window.CFSons = {
    ids: Object.keys(SONS),
    tocar(ctx, id) {
      try {
        if (!ctx) return;
        if (ctx.state === "suspended") ctx.resume();
        (SONS[id] || SONS.bipe)(ctx, ctx.currentTime);
      } catch (_) { /* navegador sem áudio */ }
    },
  };
})();

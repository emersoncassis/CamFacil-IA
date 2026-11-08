# Contribuir

No necesitas ser programador para ayudar: probar en campo, reportar un problema y compartir con otro técnico ya cuenta.

## Para quien programa

1. Lee el [PRD](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/PRD.md), sobre todo lo que queda **fuera del alcance**.
2. Lee el SDD del área que vas a tocar y abre un issue antes de cambios grandes.
3. Antes de cada commit:

    ```sh
    cargo fmt --all
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    ```

4. Commits en [Conventional Commits](https://www.conventionalcommits.org/es/), escritos en portugués, y en el PR el impacto en RAM, CPU y tamaño del binario.

## Reglas de la casa

- **Ser liviano es requisito.**
- **Cero scroll:** ninguna pantalla del programa, la app o el curso desplaza la página.
- Visual con los tokens de `web/tokens.css`: base negra, verde solo en detalles.
- Un cambio de arquitectura se vuelve un nuevo ADR.

## Hoja de ruta

Versión actual: **0.6.0** ([releases](https://github.com/emersoncassis/CamFacil-IA/releases)).

- [x] Núcleo, cerca virtual, formación con sello
- [x] Telegram, WhatsApp, red y diagrama, modo TV, informe y backup
- [x] App Android, panel web, Agente-Cam-Elo-IA y MCP
- [ ] Modo estudio completo (chat con la IA armando la receta, pre-etiquetado de fotos)

Al contribuir, licencias tu código bajo la [AGPL-3.0](https://github.com/emersoncassis/CamFacil-IA/blob/main/LICENSE).

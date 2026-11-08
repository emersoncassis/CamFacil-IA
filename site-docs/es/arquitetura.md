# Cómo funciona

Todo corre en la máquina del cliente, sin nube obligatoria. La IA analiza solo el stream liviano de la cámara.

```mermaid
flowchart LR
  subgraph F[Fuentes de video]
    C1[Cámaras IP<br/>RTSP / ONVIF]
    C2[Webcam]
    C3[Video o demostración]
  end
  subgraph N[CamFácil IA · binario Rust]
    I[Ingesta<br/>substream]
    M[Módulos de IA]
    E[Eventos y reglas]
  end
  subgraph S[Salidas]
    W[Dashboard]
    T[Modo TV]
    A[App Android]
    Z[Telegram / WhatsApp]
  end
  C1 & C2 & C3 --> I --> M --> E
  E --> W & T & A & Z
```

## Las herramientas

| Herramienta | Dónde corre | Qué hace |
|---|---|---|
| **CamFácil IA** | PC, mini PC, Raspberry, TV box | Servidor local: cámaras, IA, alertas, dashboard en el navegador |
| **Agente-Cam-Elo-IA** | Dentro del programa | Se empareja con el panel por QR y envía solo el resumen |
| **Panel** | cfia.ysafe.com.br o tu servidor | Todos tus clientes, cámaras y alertas |
| **App Android** | Celular y TV box | Servidor de cámaras o visor a distancia |
| **Formación** | Navegador | Curso corto narrado, con ejercicios y sello |
| **Modo estudio** | Navegador + Google Colab | Crear y entrenar inteligencias nuevas, solo para estudio |

## Principios

- **Liviano:** núcleo en reposo por debajo de 50 MB de RAM y 2% de CPU; binario por debajo de 30 MB.
- **Sin Node, Python ni Docker** en la máquina del cliente. La interfaz es HTML, CSS y JS embebidos en el binario.
- **Offline primero:** nada esencial depende de internet.

Detalles técnicos (en portugués): [PRD](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/PRD.md) · [SDDs](https://github.com/emersoncassis/CamFacil-IA/tree/main/docs/sdd) · [ADRs](https://github.com/emersoncassis/CamFacil-IA/tree/main/docs/adr).

# Como funciona

Tudo roda na máquina do cliente, sem nuvem obrigatória. A IA analisa só o stream leve da câmera.

!!! tip "Veja por dentro: mapa interativo"
    O **[mapa interativo](https://emersoncassis.github.io/CamFacil-IA/mapa-do-codigo/)** mostra cada parte, arquivo e rota do CamFácil IA explicados em português, com cinco fluxos guiados: *Da câmera ao alerta*, *Ver ao vivo de qualquer lugar*, *Quem pode abrir a dashboard*, *IA conectada, com rédea* e *Do download à primeira regra*.

```mermaid
flowchart LR
  subgraph F[Fontes de vídeo]
    C1[Câmeras IP<br/>RTSP / ONVIF]
    C2[Webcam]
    C3[Vídeo ou demonstração]
  end
  subgraph N[CamFácil IA · binário Rust]
    I[Ingestão<br/>stream secundário]
    M[Módulos de IA]
    E[Eventos e regras]
  end
  subgraph S[Saídas]
    W[Dashboard]
    T[Modo TV]
    A[App Android]
    Z[Telegram / WhatsApp]
  end
  C1 & C2 & C3 --> I --> M --> E
  E --> W & T & A & Z
```

## As ferramentas

| Ferramenta | Onde roda | O que faz |
|---|---|---|
| **CamFácil IA** | PC, mini PC, Raspberry, TV Box | Servidor local: câmeras, IA, alertas, dashboard no navegador |
| **Agente-Cam-Elo-IA** | Dentro do programa | Pareia com o painel por QR e manda só o resumo |
| **Painel** | cfia.ysafe.com.br ou o seu servidor | Todos os seus clientes, câmeras e alertas |
| **App Android** | Celular e TV Box | Servidor das câmeras ou visualizador à distância |
| **Formação** | Navegador | Curso curto com narração, exercícios e selo |
| **Modo estúdio** | Navegador + Google Colab | Criar e treinar inteligências novas, só para estudo |

## Princípios

- **Leve:** núcleo ocioso abaixo de 50 MB de RAM e 2% de CPU; binário abaixo de 30 MB.
- **Sem Node, Python ou Docker** na máquina do cliente. A interface é HTML, CSS e JS embutidos no binário.
- **Offline primeiro:** nada essencial depende de internet.

Detalhes técnicos: [PRD](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/PRD.md) · [SDDs](https://github.com/emersoncassis/CamFacil-IA/tree/main/docs/sdd) · [ADRs](https://github.com/emersoncassis/CamFacil-IA/tree/main/docs/adr).

## Segurança por padrão

Na própria máquina tudo abre direto. De fora (celular, TV, Tailscale, ngrok), cada aparelho é liberado uma vez: com o **PIN** gerado pelo programa, que aparece só na tela da máquina, ou **autorizado na máquina**, conferindo o mesmo código dos dois lados. A IA conectada pelo MCP lê, mas não abusa, não vê segredo e nunca aciona portão.

```mermaid
flowchart LR
  M[Na própria máquina] --> D[Dashboard]
  R[Celular · TV · Tailscale · ngrok] --> P1{Host e Origin<br/>da dashboard?}
  X[Site malicioso] --> P1
  P1 -- não --> N[Recusado]
  P1 -- sim --> P2{Aparelho<br/>liberado?}
  P2 -- sim --> D
  P2 -- não --> E[PIN ou pedir à máquina] --> K[Máquina confere o código] --> D
  IA[Assistente de IA] --> T{Token e limite<br/>por minuto}
  T -- sim --> S[Só leitura · nunca portão]
  T -- não --> N
```

PIN fraco (123456, datas, repetições) é recusado com explicação, e os erros travam a entrada aos poucos. Tudo fica no registro da aba **Segurança**. Detalhes no [documento de segurança](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/seguranca/SEGURANCA.md).

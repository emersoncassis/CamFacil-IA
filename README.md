<div align="center">

<img src="https://capsule-render.vercel.app/api?type=waving&color=0:163134,50:035144,100:06CB3F&height=220&section=header&text=CamF%C3%A1cil%20IA&fontSize=64&fontColor=EFEFED&animation=fadeIn&fontAlignY=38&desc=Intelig%C3%AAncia%20de%20v%C3%ADdeo%20para%20quem%20instala%20c%C3%A2meras&descAlignY=58&descSize=18" width="100%" alt="CamFácil IA"/>

<a href="#-instale-em-um-comando">
  <img src="https://readme-typing-svg.demolab.com?font=Fira+Code&weight=600&size=22&pause=1200&color=06CB3F&center=true&vCenter=true&width=720&lines=IA+de+v%C3%ADdeo+em+qualquer+m%C3%A1quina%2C+com+um+comando.;Aprenda+IA+e+rede+IP+fazendo.;Valide+no+cliente.+Venda+a+c%C3%A2mera+com+IA.;Feito+por+instalador%2C+para+instalador." alt="Frases animadas"/>
</a>

<p>
  <img src="https://img.shields.io/badge/Rust-leve%20e%20r%C3%A1pido-06CB3F?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/licen%C3%A7a-AGPL--3.0-035144?style=for-the-badge" alt="AGPL-3.0"/>
  <img src="https://img.shields.io/badge/open%20source-100%25-85FFC5?style=for-the-badge&labelColor=163134" alt="Open source"/>
</p>
<p>
  <img src="https://img.shields.io/badge/Windows-PC%20velho%20ok-163134?style=flat-square&logo=windows&logoColor=white" alt="Windows"/>
  <img src="https://img.shields.io/badge/Linux-mini%20PC-163134?style=flat-square&logo=linux&logoColor=white" alt="Linux"/>
  <img src="https://img.shields.io/badge/Raspberry%20Pi-ARM64-163134?style=flat-square&logo=raspberrypi&logoColor=white" alt="Raspberry Pi"/>
  <img src="https://img.shields.io/badge/Android-TV%20Box%20(em%20breve)-163134?style=flat-square&logo=android&logoColor=white" alt="Android"/>
  <img src="https://img.shields.io/github/stars/emersoncassis/CamFacil-IA?style=flat-square&color=06CB3F" alt="Estrelas"/>
</p>

</div>

---

> [!IMPORTANT]
> **O CamFácil IA é um ambiente de aprendizagem e validação.** A solução final no seu cliente é uma câmera ou gravador com **IA de borda**, ou uma plataforma corporativa de IA em nuvem. Aqui você aprende, prova o valor e vende com segurança.

## 🎯 Por que existe

Técnico bom trava em duas coisas: **inteligência artificial** e **rede IP**. O CamFácil IA ensina as duas na prática, rodando no computador que já está no balcão do seu cliente. Leia o [manifesto](MANIFESTO.md).

<!-- Substituir pelo GIF real da dashboard quando a fase 2 estiver pronta -->
<div align="center">
  <img src="docs/assets/demo-placeholder.svg" width="90%" alt="Dashboard do CamFácil IA: balão de inteligência piscando quando alguém cruza a cerca virtual"/>
</div>

## ⚡ Instale em um comando

```sh
# Linux / Raspberry Pi (em breve)
curl -fsSL https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/install.sh | sh
```

```powershell
# Windows (em breve)
irm https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/install.ps1 | iex
```

Ou baixe o `camfacil.exe` em **[Releases](https://github.com/emersoncassis/CamFacil-IA/releases)**, dê dois cliques e abra `http://localhost:7070`.

## 🧠 Níveis de IA

O CamFácil IA testa a sua máquina e libera só o que ela aguenta.

| Nível | Módulos | Roda em |
|:---:|---|---|
| 🟢 **Leve** | Movimento inteligente · Cerca e linha virtual · Saúde das câmeras | Quase qualquer PC |
| 🟡 **Médio** | Pessoas e veículos · Contagem · Permanência · Mapa de calor | PC de 4 núcleos, mini PC |
| 🔴 **Pesado** | Leitura de placas · Reconhecimento facial | Máquinas mais fortes |

## 🧰 O que vem na caixa

| | |
|---|---|
| 🎓 **Aula em cada clique** | Agente guia com voz explica cada botão, mostra o certo e o erro comum, e propõe um desafio |
| 🔎 **Acha as câmeras sozinho** | Busca ONVIF na rede e gerador de RTSP por fabricante |
| 🩺 **Diagnóstico de rede** | Descobre câmera que trocou de IP pelo MAC, conflito de IP e senha padrão |
| 🗺️ **Diagrama da rede** | Desenha o unifilar da instalação e explica em voz |
| 💬 **Telegram e WhatsApp** | Alertas e comandos com QR code |
| 📺 **Modo TV** | Tela de quiosque para o cliente, com a sua logo |
| 📱 **App Android** | Ao vivo, alertas e botões para portão e relé, via Tailscale |
| 📊 **Relatório de validação** | PDF com a sua marca para fechar a venda |
| 🤖 **MCP e IA** | Configure tudo conversando com uma IA (opcional) |

## 🧑‍🔧 Para você, técnico

Você não precisa ser programador. Você precisa de um computador velho, uma câmera (ou só a webcam) e vontade de aprender. Em troca, ganha **um novo serviço para vender**, **conhecimento que nenhum curso entregou na prática** e **liberdade** para escolher a melhor solução para cada cliente.

## 🛠️ Para quem programa

- Produto: [`docs/PRD.md`](docs/PRD.md) · Arquitetura: [`docs/sdd/`](docs/sdd) · Decisões: [`docs/adr/`](docs/adr) · Visual: [`docs/DESIGN_SYSTEM.md`](docs/DESIGN_SYSTEM.md)
- Desenvolvido com [Claude Code](https://claude.com/claude-code): veja [`CLAUDE.md`](CLAUDE.md), os agentes em `.claude/agents/` e as skills em `.claude/skills/`.
- Quer criar um módulo de IA? Comece por [`CONTRIBUTING.md`](CONTRIBUTING.md).

## 🗺️ Roadmap

- [ ] **Fase 1** — Núcleo: binário, dashboard, benchmark, ONVIF, gerador RTSP
- [ ] **Fase 2** — Cerca virtual com balões e aula guiada
- [ ] **Fase 3** — Telegram, WhatsApp e comandos
- [ ] **Fase 4** — Diagnóstico de rede e diagrama unifilar
- [ ] **Fase 5** — Modo TV, monitor de hardware, relatório e backup
- [ ] **Fase 6** — App Android, web app e MCP

## 🤝 Contribua

<a href="https://github.com/emersoncassis/CamFacil-IA/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=emersoncassis/CamFacil-IA" alt="Contribuidores"/>
</a>

<div align="center">

<sub>Identidade visual inspirada na paleta da Intelbras, onde este projeto nasceu. Projeto independente, sem vínculo com a Intelbras.</sub>

<img src="https://capsule-render.vercel.app/api?type=waving&color=0:06CB3F,50:035144,100:163134&height=120&section=footer" width="100%" alt=""/>

</div>

# Primeiros passos

O programa no computador do cliente é só **operação**. Quem quer aprender do zero começa pela [Formação de IA para CFTV](formacao.md), no navegador.

## Ligar a primeira câmera

1. **Abra o programa.** Ele abre direto em **Câmeras** e procura sozinho na rede (busca ONVIF).
2. **Escolha a câmera**, digite usuário e senha e clique em conectar. A imagem aparece na hora.
3. **Ligue a IA quando quiser**: desenhe a linha ou a zona, ou peça *"IA, sugira para mim"*.

Não tem câmera por perto? Use a **webcam**, um **arquivo de vídeo** ou a **cena de demonstração**.

!!! tip "Login salvo"
    Câmeras que usam o mesmo usuário e senha entram de uma vez. A senha fica cifrada na máquina.

## O que o cliente vê

O cliente nunca vê caixas ou riscos da IA. Ele vê só o **balão com o nome da regra**, que pisca no alerta, com som e tela piscando.

## O que vem na caixa

| Recurso | O que faz |
|---|---|
| **Ao vivo** | Mosaico de 1 a 4 câmeras e, embaixo, o status: aparelho, IA, alertas e serviços |
| **Diagnóstico de rede** | Acha câmera que trocou de IP pelo MAC, conflito de IP e senha padrão |
| **Diagrama da rede** | Desenha o unifilar da instalação e explica em voz |
| **Telegram e WhatsApp** | Alertas e comandos com QR code (WhatsApp pela Evolution API, num chip dedicado) |
| **Modo TV** | Tela de quiosque para o cliente, com a sua logo |
| **App Android** | Vira o servidor das câmeras ou vê um cliente à distância pelo Tailscale |
| **Relatório de validação** | PDF com a sua marca para fechar a venda |
| **MCP** | Configure tudo conversando com uma IA (desligado por padrão) |
| **Modo estúdio** *(básico)* | Descreva uma inteligência nova, rotule, treine no Colab e compartilhe (só estudo) |

??? question "O programa fica aberto o tempo todo?"
    Dá para deixar abrindo junto com o computador. A janela do terminal mostra o resumo ao vivo: CPU, memória e câmeras.

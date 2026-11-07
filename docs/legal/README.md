# LGPD e responsabilidades

> Estes modelos são um ponto de partida, **não são consultoria jurídica**. O técnico deve revisá-los com um advogado antes de usar.

## Base legal (situação em novembro de 2026)
- Lei 13.709/2018 (LGPD) continua sendo a lei aplicável.
- A ANPD passou a ser Agência Nacional de Proteção de Dados (Lei 15.352/2026) e mantém biometria, reconhecimento facial e IA entre as prioridades de regulação e fiscalização para 2026–2027. Ainda não há regulamento final específico para IA, mas a LGPD já se aplica.
- Biometria e reconhecimento facial são dados sensíveis (art. 11). Direitos do titular estão no art. 18.

## Documentos
| Documento | Quando usar |
|---|---|
| [Isenção de responsabilidade do projeto](ISENCAO.md) | Sempre: README, primeira execução (com aceite) e rodapé da dashboard |
| [Termo de responsabilidade do técnico](TERMO-RESPONSABILIDADE-TECNICO.md) | Antes de ativar Telegram/WhatsApp com imagens, modo estúdio ou envio de imagens a provedores de IA |
| [Termo de autorização do cliente](TERMO-AUTORIZACAO-CLIENTE.md) | Antes de coletar imagens do cliente para estudo ou treinamento |
| [Placa "ambiente monitorado por IA"](PLACA-AMBIENTE-MONITORADO.md) | No local durante a validação |

## Boas práticas embutidas no software
- Dados ficam na máquina do cliente; o projeto não hospeda imagens.
- Modo LGPD: rostos borrados no que sai da máquina.
- Prazos de exclusão configuráveis (clipes, arquivo do Telegram, fotos do modo estúdio).
- Reconhecimento facial nunca é ligado por padrão e é bloqueado no modo estúdio.
- Cada aceite de termo fica registrado com data, hora e versão.

# SDD-02 — Módulos de IA

Status: proposto · Fase 2

## Objetivo
Definir um contrato simples para que qualquer pessoa da comunidade crie um módulo de IA, e o núcleo consiga baixar, ligar e desligar módulos conforme a máquina aguenta.

## Contrato de módulo

Cada módulo é uma pasta com um `module.toml`:

```toml
id = "cerca-virtual"
nome = "Cerca virtual"
nivel = "leve"            # leve | medio | pesado
versao = "0.1.0"
licenca = "AGPL-3.0"

[modelo]                  # opcional (módulos leves podem não ter modelo)
arquivo = "yolo-nano.onnx"
url = "https://github.com/.../releases/download/models-v1/yolo-nano.onnx"
sha256 = "..."
entrada = [1, 3, 320, 320]

[recursos]
ram_mb = 60
fps_padrao = 5

[aula]
audios = ["intro.ogg", "erro-comum.ogg", "desafio.ogg"]
desafio = "desafio.toml"
```

E implementa o trait:

```rust
pub trait Module: Send {
    fn id(&self) -> &str;
    fn configure(&mut self, rules: &[Rule]) -> Result<()>;
    fn process(&mut self, frame: &Frame, ctx: &mut Ctx) -> Result<Vec<Event>>;
}
```

`Event` tem: regra, câmera, tipo (`entrou`, `saiu`, `contagem`, `permanencia`, `estado`), valor e instante. O núcleo cuida de snapshot, clipe, balão e notificação; o módulo só detecta.

## Inferência
- `ort` (ONNX Runtime) com provedor CPU por padrão; DirectML/CUDA/CoreML opcionais via feature.
- Uma sessão por modelo, compartilhada entre câmeras; fila de quadros com descarte (nunca acumular atraso).
- Pré-processamento em `image`/`fast_image_resize`.

## Módulos iniciais
| Módulo | Técnica | Nível |
|---|---|---|
| Movimento inteligente | diferença de quadros + filtro de área/tempo | leve |
| Cerca/linha virtual | polígonos e linhas sobre movimento ou detecção | leve (movimento) / médio (pessoa) |
| Pessoas e veículos | YOLO nano ONNX + rastreamento simples (IoU/ByteTrack) | médio |
| Contagem e permanência | sobre o rastreamento | médio |
| Placas | detector + OCR leve | pesado |
| Face | YuNet + embedding | pesado |
| Saúde da câmera | brilho, contraste, nitidez (Laplaciano), sem sinal | leve · **implementado** |
| Mapa de calor | acumulação de posições | médio |

## Cerca virtual genérica
- Regra = `{ nome, camera, forma: poligono|linha, gatilho, estados[], agenda }`.
- Estados editáveis (ex.: `vazia`/`cheia`, texto e cor).
- O balão mostra o nome da regra; em alerta muda de cor e pisca (Design System).
- A detecção nunca é desenhada para o cliente (sem caixas do YOLO).

## Níveis e benchmark
O benchmark roda um modelo de referência por N segundos e mede quadros/s. O nível liberado é o maior em que o módulo de referência fica acima de 3 fps por câmera.

## Critérios de aceite (fase 2)
- Cerca virtual funciona em webcam e RTSP, com balão e alerta.
- Desligar o módulo libera a RAM do modelo.
- Um módulo de exemplo da comunidade (`examples/modulo-exemplo`) compila e é carregado.

## Saúde da câmera (status: implementado · fase 3)
- `cf-core/src/saude.rs`: `classificar(quadro)` → `ok | escura | coberta | desfocada`; `Monitor` com histerese por tempo (4 s): a nova situação precisa se sustentar a histerese inteira para virar mudança. Uma piscada (pessoa na frente da lente, farol) não gera evento. Testes unitários cobrem a histerese.
- `sem_sinal`: a thread da câmera alimenta o monitor a cada segundo enquanto o ffmpeg está caído; segue a mesma histerese.
- Evento `saude` em `eventos` **só na mudança confirmada**, com `detalhe` = situação nova (inclusive a volta para `ok`). Sem spam.
- Dashboard: tag de saúde na tabela "Suas câmeras" e no cabeçalho do ao vivo; balão `Imagem escura` / `Câmera coberta` / `Imagem desfocada` sobre o vídeo (pisca como alerta; sem sinal usa a cortina do próprio vídeo); bloco "Saúde das câmeras" na tela Início (`GET /api/saude`) com situação, há quanto tempo, último problema e últimas mudanças; som opcional por câmera (chave `saude:<camera_id>`, no navegador).
- Custo: uma avaliação por segundo por câmera em quadro 160 px; sem impacto medido no ocioso.

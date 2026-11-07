# SDD-02 — Módulos de IA

Status: leve e médio **implementados** (fases 2 e 3); pesado fora do programa local (PRD §6)

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
| Pessoas e veículos | YOLO11n ONNX + rastreamento simples por proximidade | médio · **implementado** |
| Contagem e permanência | sobre o rastreamento | médio · **implementado** |
| Placas | detector + OCR leve | pesado · fica para câmera/gravador com IA de borda ou parceiro |
| Face | YuNet + embedding | pesado · idem |
| Saúde da câmera | brilho, contraste, nitidez (Laplaciano), sem sinal | leve · **implementado** |
| Mapa de calor | acumulação de posições | médio · **implementado** |

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

## Nível médio (status: implementado · fase 3)
- **Runtime**: `ort` 2.0.0-rc.15 com `default-features = false, features = ["load-dynamic"]` (sem `ndarray`, sem download no build). A `libonnxruntime` **1.20.1** vem das releases oficiais `microsoft/onnxruntime` (linux x64/aarch64, windows x64/arm64, macOS arm64/x86_64), conferida por SHA-256 fixo em `crates/camfacil/src/ia/mod.rs`; só a biblioteca é extraída (tar/zip lidos à mão + `miniz_oxide`) para `data_dir/modelos/onnxruntime/`. `CAMFACIL_ORT_LIB` aponta para um runtime já instalado.
- **Modelo**: `yolo11n.onnx` (640, opset 12, sem NMS) — ver `modelos/README.md`. URL fixa na release `modelos-v1` do repositório, fallback no branch `main`; SHA-256 em `modelos/yolo11n.onnx.sha256` (embutido no binário). Cópia por pendrive para `data_dir/modelos/` funciona offline.
- **Download sob demanda**: ao ligar o primeiro módulo médio a dashboard chama `POST /api/ia/baixar`; `GET /api/ia` dá fase, bytes e erro; `POST /api/ia/cancelar`; `DELETE /api/ia` apaga. Barra de progresso e cancelar na tela Módulos de IA. Destravar exige `bench_level` ≥ médio (teste da máquina); o servidor também recusa.
- **Detector** (`ia/detector.rs`): uma thread, uma sessão compartilhada, 2 threads intra-op, ≤ 3 inferências/s no total (intervalo 340 ms), sempre no quadro já reduzido do stream secundário (JPEG → RGB → letterbox 640). Dorme sem módulo médio ligado e libera o modelo da RAM após 1 min ocioso.
- **Pós-processamento** (`cf-core/src/deteccao.rs`, com testes): classes pessoa, bicicleta, carro, moto, ônibus, caminhão; NMS; rastreamento por proximidade do pé; contagem por linha com sentido; zona ocupada por pessoa; permanência (pessoa parada > N s, N em `settings.permanencia_s`, padrão 60); mapa de calor em grade 32×18.
- **Módulos**: `pessoas` (balão "IA viu: 4 pessoas · 1 ônibus"; zonas passam a usar pessoa em vez de movimento), `contagem` (linhas passam a contar pessoas/veículos detectados; o analisador por movimento solta essas regras para não contar duas vezes), `permanencia` (evento `permanencia`, balão com "parado há N s", alerta no Telegram com antirrepique `permanencia:<regra>`), `calor` (`GET/DELETE /api/cameras/{id}/calor`; camada que só o técnico liga no ao vivo). **Nunca caixas**: só balões e tags.
- **Medido** (release, Xeon 2,1 GHz, 2 núcleos, 2 câmeras): ocioso 4,6 MB binário · 6,4 MB RAM · 0 % CPU; só leve com 2 câmeras 8,6 MB · 6 %; com detector ligado 134 MB RAM · ~55 % de um núcleo · 85 ms por inferência. O orçamento do ocioso continua; a IA média custa o que custa quando ligada.
- Falta validar: download do modelo pela URL da release (a release `modelos-v1` ainda precisa ser publicada), Windows/macOS/Raspberry (aqui só linux x64), e máquinas fracas (o teste da máquina é quem destrava).

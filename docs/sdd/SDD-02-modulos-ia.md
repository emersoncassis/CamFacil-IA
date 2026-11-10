# SDD-02 — Módulos de IA

Status: leve e médio **implementados** (fases 2 e 3, evoluídos até a 0.9.x); pesado fora do programa local (PRD §6) · o contrato de módulo por pasta (abaixo) continua **proposta** · revisado em 2026-11-10

## Objetivo
Definir um contrato simples para que qualquer pessoa da comunidade crie um módulo de IA, e o núcleo consiga baixar, ligar e desligar módulos conforme a máquina aguenta.

## Contrato de módulo (proposta, não implementada)

> **Situação real:** não existe `module.toml`, trait `Module`, pasta `modules/` nem `examples/modulo-exemplo`. Os módulos de hoje são fixos no programa: a lista `MODULOS` de `crates/camfacil/src/video.rs` (movimento, cerca, saúde, alertas da câmera, pessoas e veículos, contagem, permanência, calor) e as inteligências por regra de `crates/cf-core/src/inteligencias.rs` ([SDD-09](SDD-09-inteligencias-por-regra.md)). O caminho real para a comunidade criar uma inteligência nova **sem compilar o programa** é a **receita do modo estúdio** ([SDD-07](SDD-07-modo-estudio.md), ADR 0013). Este contrato fica como desenho de uma possível abertura futura de módulos compilados.

Cada módulo seria uma pasta com um `module.toml`:

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
- `ort` (ONNX Runtime) com `load-dynamic`, **só provedor CPU** (DirectML/CUDA/CoreML não foram ligados; seguem como ideia).
- Uma sessão por modelo, compartilhada entre câmeras; fila de quadros com descarte (nunca acumular atraso).
- Pré-processamento próprio: JPEG decodificado com `zune-jpeg`, reduzido e com letterbox 640 em `ia/detector.rs` (sem `image` nem `fast_image_resize`).

## Módulos iniciais
| Módulo | Técnica | Nível |
|---|---|---|
| Movimento inteligente | diferença de quadros + filtro de área/tempo | leve · **implementado** |
| Cerca/linha virtual | polígonos e linhas sobre movimento ou detecção | leve (movimento) / médio (pessoa) · **implementado** |
| Alertas da câmera | eventos que a própria câmera/gravador manda (Intelbras/Dahua, Hikvision) | leve · **implementado** (validado só com servidor simulado; [SDD-09](SDD-09-inteligencias-por-regra.md)) |
| Pessoas e veículos | YOLO11n ONNX + rastreamento simples por proximidade | médio · **implementado** |
| Contagem e permanência | sobre o rastreamento | médio · **implementado** |
| Placas | detector + OCR leve | pesado · fica para câmera/gravador com IA de borda ou parceiro |
| Face | YuNet + embedding | pesado · idem |
| Saúde da câmera | brilho, contraste, nitidez (Laplaciano), sem sinal | leve · **implementado** |
| Mapa de calor | acumulação de posições | médio · **implementado** |

## Cerca virtual genérica
- Regra (`RegraSalva` em `cf-core/src/store.rs`) = `{ nome, camera, forma: zona|linha, pontos normalizados 0–1, rotulo_on, rotulo_off, ajustes }`, com `ajustes` = `{ intel, classes, parametro, cor, som, ativa, espera_modulo }` ([SDD-09](SDD-09-inteligencias-por-regra.md)). Regra pode ser ligada e desligada sem excluir.
- Estados editáveis: os dois textos (ex.: `vazia`/`cheia`), mais cor e som por regra.
- **Agenda (dias e horários) e gatilhos compostos não foram implementados** (PRD §5.2 "regras por horário"; previsto no [SDD-10](SDD-10-ia-avancada.md)).
- O balão mostra o nome da regra; em alerta muda de cor e pisca (Design System).
- A detecção nunca é desenhada para o cliente (sem caixas do YOLO).

## Níveis e benchmark
O benchmark roda um modelo de referência por N segundos e mede quadros/s. O nível liberado é o maior em que o módulo de referência fica acima de 3 fps por câmera.

**Situação:** o teste da máquina de hoje (`cf-core/src/bench.rs`) é uma estimativa por kernel de ponto flutuante, com limites provisórios, e grava `bench_level`; destravar o nível médio exige esse nível e o servidor também recusa. A troca pela inferência real do modelo de referência segue pendente. O **teste de estresse** (`crates/camfacil/src/estresse.rs`, `POST /api/estresse`) já mede de verdade quantas câmeras de demonstração com cerca virtual a máquina aguenta (degraus de 1, 2, 3, 4, 6, 8… câmeras, CPU e memória por degrau).

## Critérios de aceite (fase 2)
- Cerca virtual funciona em webcam e RTSP, com balão e alerta. **Feito.**
- Desligar o módulo libera a RAM do modelo (o detector dorme sem módulo médio ligado e descarrega o modelo após 1 min ocioso). **Feito.**
- Um módulo de exemplo da comunidade (`examples/modulo-exemplo`) compila e é carregado. **Não feito** (depende do contrato por pasta; ver acima). A via equivalente já funcionando é a receita do estúdio.

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

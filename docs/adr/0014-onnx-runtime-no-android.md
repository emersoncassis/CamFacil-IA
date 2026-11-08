# ADR 0014 · ONNX Runtime no app Android

Status: aceita · 2026-11-08

## Contexto
O nível médio (detecção de pessoas com YOLO) usa o ONNX Runtime carregado em tempo de execução (`ort`, `load-dynamic`). No PC o programa baixa o pacote oficial do GitHub da Microsoft. Para Android a Microsoft não publica esse pacote; por isso o servidor no celular/TV Box mostrava "não há ONNX Runtime oficial para este sistema" e só o nível leve funcionava.

## Decisão
O APK passa a depender do pacote **oficial** da Microsoft para Android no Maven Central, `com.microsoft.onnxruntime:onnxruntime-android` (1.20.0, mesma linha do PC). O Gradle extrai `libonnxruntime.so` para o `nativeLibraryDir`; a ponte Java (`libonnxruntime4j_jni.so`) fica fora. O serviço do servidor passa `CAMFACIL_ORT_LIB` ao núcleo, que já aceitava essa variável. Só ARM entra no APK (`arm64-v8a`, `armeabi-v7a`). O modelo `yolo11n.onnx` continua sendo baixado uma vez, como no PC.

## Consequências
- O APK cresce alguns MB (a biblioteca do runtime por arquitetura). Se pesar, o próximo passo é um APK por arquitetura.
- Nenhum código novo de inferência: o mesmo detector do PC roda no aparelho.
- Alternativas descartadas: `tract` (Rust puro, mais lento e com operadores faltando para YOLO11) e NNAPI/LiteRT (exigiria converter o modelo e outro caminho de código).

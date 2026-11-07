# Modelos do nível médio

| Arquivo | Origem | Tamanho | SHA-256 |
|---|---|---|---|
| `yolo11n.onnx` | YOLO11n (Ultralytics 8.4.207, AGPL-3.0) exportado em ONNX, entrada 640×640, opset 12, sem NMS embutido, simplificado com onnxslim | 10,7 MB | em `yolo11n.onnx.sha256` |

Comando usado na exportação (Python só na máquina de quem publica; o programa nunca precisa dele):

```sh
pip install ultralytics onnx onnxslim
python -c "from ultralytics import YOLO; YOLO('yolo11n.pt').export(format='onnx', imgsz=640, opset=12, simplify=True, dynamic=False)"
```

O programa baixa este arquivo sob demanda de `https://github.com/emersoncassis/CamFacil-IA/releases/download/modelos-v1/yolo11n.onnx` (fallback: este arquivo no branch `main`) e confere o SHA-256 gravado em `crates/camfacil/src/ia/mod.rs` (`include_str!` deste `.sha256`). **Ao publicar uma versão nova do modelo**: troque os dois arquivos aqui, crie/atualize a release `modelos-v1` com o `.onnx` anexado, e o binário passa a aceitar só o novo hash.

Quem está offline pode copiar `yolo11n.onnx` por pendrive para `<pasta de dados>/modelos/` (Windows `%PROGRAMDATA%\CamFacil\modelos`, Linux `~/.local/share/camfacil/modelos`); o SHA-256 é conferido na carga.

O ONNX Runtime (1.20.1) vem das releases oficiais de `microsoft/onnxruntime`; só a biblioteca (`libonnxruntime.so` / `onnxruntime.dll` / `libonnxruntime.dylib`) é extraída para `<pasta de dados>/modelos/onnxruntime/`.

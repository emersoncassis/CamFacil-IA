# O que vem na caixa

Tudo o que o CamFácil IA já faz, funcionalidade por funcionalidade. O resumo está no [README](../README.md); os detalhes técnicos, nos [SDDs](sdd/).

| | | |
|:---:|---|---|
| <img src="assets/icons/aula.svg" width="20" alt=""/> | **Aula em cada passo** | A [Formação de IA para CFTV](https://cfia.ysafe.com.br/aprender) fica no site, com a voz do autor, animações Errado × Certo e exercícios; no programa, os botões **Aprender** abrem a lição certa, e a câmera de demonstração usa a mesma loja do curso |
| <img src="assets/icons/lupa.svg" width="20" alt=""/> | **Acha as câmeras sozinho** | Busca ONVIF na rede e gerador de RTSP por fabricante; login salvo (cifrado) para as câmeras que usam o mesmo usuário e senha; editar e remover câmera; reconhece gravador (DVR/NVR) e deixa escolher um ou vários canais |
| <img src="assets/icons/tv.svg" width="20" alt=""/> | **Ao vivo** | Mosaico de 1, 2, 3 ou 4 câmeras e, embaixo, o status: aparelho, IA (regras ativas, modelos rodando), alertas, para quem vai cada alerta e serviços ativos |
| <img src="assets/icons/rede.svg" width="20" alt=""/> | **Diagnóstico de rede** | Descobre câmera que trocou de IP pelo MAC, conflito de IP, câmera instável, gateway ou DNS fora e rede lenta (senha de fábrica ainda não) |
| <img src="assets/icons/rede.svg" width="20" alt=""/> | **Autocorreção de IP** | Opção por câmera: se o DHCP mudar o IP, o programa reconhece a câmera pelo MAC, troca o endereço sozinho, confere a imagem e avisa |
| <img src="assets/icons/diagrama.svg" width="20" alt=""/> | **Diagrama da rede** | Desenha o unifilar da instalação em SVG, com IP e MAC, para baixar |
| <img src="assets/icons/mensagem.svg" width="20" alt=""/> | **Telegram e WhatsApp** | Alertas e comandos com QR code |
| | ↳ WhatsApp usa uma [Evolution API](https://github.com/EvolutionAPI/evolution-api) que você sobe no mesmo servidor Docker do painel: `docker run -d -p 8080:8080 -e AUTHENTICATION_API_KEY=troque atendai/evolution-api:v2.2.3` e aponte a URL na dashboard (chip dedicado, nunca o seu número) | SDD-04 |
| <img src="assets/icons/tv.svg" width="20" alt=""/> | **Modo TV** | Tela de quiosque para o cliente, com a sua logo e o QR do seu WhatsApp; até 9 câmeras por página, trocando sozinha |
| <img src="assets/icons/celular.svg" width="20" alt=""/> | **App Android** | Servidor no próprio celular ou TV Box (com IA de pessoas, YOLO), endereço da rede da loja com QR e botão do Tailscale; ou visualizador à distância, com alertas |
| <img src="assets/icons/relatorio.svg" width="20" alt=""/> | **Relatório de validação** | PDF com o seu nome e o do cliente, foto de cada câmera, alertas da semana, saúde da máquina e proposta, para fechar a venda |
| <img src="assets/icons/robo.svg" width="20" alt=""/> | **MCP e IA** | Configure tudo conversando com uma IA (opcional) |
| <img src="assets/icons/chip.svg" width="20" alt=""/> | **Modo estúdio** *(versão básica)* | **Treine sua IA**: descreva uma inteligência nova, rotule no navegador, treine no Colab, rode no programa e compartilhe com outros técnicos (só estudo). **VMS sob medida**: em breve |
| <img src="assets/icons/rede.svg" width="20" alt=""/> | **Agente-Cam-Elo-IA** | O elo entre o programa do cliente e o seu painel: pareia por QR e manda só o resumo (o vídeo nunca passa pela nuvem) |
| <img src="assets/icons/chip.svg" width="20" alt=""/> | **Nada vem ligado** | Todo módulo começa desligado; regra criada com o módulo desligado fica salva desligada e liga junto com ele |
| <img src="assets/icons/lupa.svg" width="20" alt=""/> | **Alertas da própria câmera** | Escuta os eventos que a câmera ou o gravador manda (Intelbras/Dahua e Hikvision) e registra como "recebido da câmera" |
| <img src="assets/icons/chave.svg" width="20" alt=""/> | **Portão e relé** | Até 12 botões com PIN, código de uso único de 20 s e log de quem acionou; o MCP nunca aciona |
| <img src="assets/icons/chip.svg" width="20" alt=""/> | **Monitor de hardware** | CPU, RAM, disco e temperatura a cada 60 s, histórico de 30 dias e alerta quando passa do limite |
| <img src="assets/icons/chip.svg" width="20" alt=""/> | **Teste de estresse** | Sobe câmeras de teste até a máquina mostrar o limite: diz quantas câmeras ela aguenta |
| <img src="assets/icons/chave.svg" width="20" alt=""/> | **Backup cifrado** | Exporta e importa toda a configuração num arquivo `.cfbkp`, também pelo Telegram |

# ADR-0010 — WhatsApp pela Evolution API (gateway HTTP), não por biblioteca embutida

Status: aceito · 2026-11-07 · substitui a ADR-0004

## Contexto
A ADR-0004 previa `whatsapp-rust` ou `whatsmeow` dentro do (ou ao lado do) binário. Na prática: o protocolo muda com frequência, o port Rust é instável e um processo Go auxiliar fere a ideia de binário único. Ao mesmo tempo, quem usa o CamFácil IA já tem um servidor Docker para o painel (`cfia.ysafe.com.br` ou a própria VPS).

## Decisão
- O programa local **não** embute biblioteca não oficial do WhatsApp. Ele fala HTTP com uma **Evolution API** (contêiner Docker) que o técnico sobe onde quiser: mesmo servidor do painel, VPS ou um PC da rede.
- Chave da Evolution cifrada pelo mesmo cofre do token do Telegram; comandos chegam por webhook (`/api/whatsapp/webhook/{token}`).
- Aviso permanente na tela: número pode ser bloqueado; chip dedicado, nunca o número principal.

## Consequências
- Regra 2 (sem Node/Docker no caminho de execução) continua valendo **para o programa local**; o Docker fica no servidor, opcional, como já é o painel.
- Quando a Evolution quebrar por mudança do WhatsApp, basta atualizar a imagem; o binário não muda.
- Sem Evolution alcançável, o WhatsApp simplesmente não liga; Telegram segue como canal padrão.

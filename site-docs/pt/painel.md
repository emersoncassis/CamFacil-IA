# Painel no seu servidor

O painel web reúne os seus clientes: login com Google, pareamento dos computadores por QR, câmeras no ar, alertas e *ver ao vivo* pelo Tailscale. O oficial roda em [cfia.ysafe.com.br](https://cfia.ysafe.com.br), mas você pode subir o seu.

O vídeo **nunca passa pela nuvem**: o Agente-Cam-Elo-IA manda só o resumo (câmeras, máquina, alertas).

## Em qualquer VPS Linux

Hostinger, HostGator, Google Cloud, AWS, Oracle, DigitalOcean, Contabo…

1. **Um comando na VPS** (instala o Docker se faltar, pergunta o domínio, gera a senha do banco e liga HTTPS):

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar-painel.sh | sudo sh
    ```

2. **DNS:** crie um registro **A** do seu domínio apontando para o IP da VPS. As portas 80 e 443 precisam estar livres.
3. **Login com Google:** no Google Cloud, crie um **ID do cliente OAuth** do tipo *Aplicativo da Web* com o redirecionamento `https://SEU-DOMINIO/api/auth/google/callback`. Coloque o ID e a chave em `/opt/camfacil-painel/.env` e rode `docker compose up -d`.
4. **Aponte os agentes:** no computador do cliente, defina `CAMFACIL_SITE=https://SEU-DOMINIO`.

??? example "Testar no seu PC com Docker"
    ```sh
    mkdir camfacil-painel && cd camfacil-painel
    curl -fsSL https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/deploy/docker-compose.vps.yml -o docker-compose.yml
    printf 'DOMINIO=localhost\nPOSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 20)" > .env
    docker compose up -d
    ```

Imagem pronta: `ghcr.io/emersoncassis/camfacil-ia-painel:latest`. Para Dokploy, Coolify ou Portainer, use `deploy/docker-compose.dokploy.yml` ([guia de deploy](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/DEPLOY.md)).

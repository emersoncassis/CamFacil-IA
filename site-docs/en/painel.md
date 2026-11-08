# Your own dashboard

The web dashboard brings your customers together: Google login, pairing computers by QR code, cameras online, alerts and *watch live* through Tailscale. The official one runs at [cfia.ysafe.com.br](https://cfia.ysafe.com.br), but you can host your own.

Video **never goes through the cloud**: Agente-Cam-Elo-IA sends only a summary (cameras, machine, alerts).

## On any Linux VPS

Hostinger, HostGator, Google Cloud, AWS, Oracle, DigitalOcean, Contabo…

1. **One command on the VPS** (installs Docker if missing, asks for the domain, generates the database password and turns on HTTPS):

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar-painel.sh | sudo sh
    ```

2. **DNS:** create an **A** record for your domain pointing to the VPS IP. Ports 80 and 443 must be free.
3. **Google login:** in Google Cloud, create an **OAuth client ID** of type *Web application* with the redirect `https://YOUR-DOMAIN/api/auth/google/callback`. Put the ID and secret in `/opt/camfacil-painel/.env` and run `docker compose up -d`.
4. **Point the agents:** on the customer's computer, set `CAMFACIL_SITE=https://YOUR-DOMAIN`.

??? example "Try it on your PC with Docker"
    ```sh
    mkdir camfacil-painel && cd camfacil-painel
    curl -fsSL https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/deploy/docker-compose.vps.yml -o docker-compose.yml
    printf 'DOMINIO=localhost\nPOSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 20)" > .env
    docker compose up -d
    ```

Prebuilt image: `ghcr.io/emersoncassis/camfacil-ia-painel:latest`. For Dokploy, Coolify or Portainer, use `deploy/docker-compose.dokploy.yml` ([deploy guide](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/DEPLOY.md), in Portuguese).

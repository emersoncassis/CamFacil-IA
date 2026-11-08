# Panel en tu servidor

El panel web reúne a tus clientes: login con Google, emparejamiento de computadoras por QR, cámaras en línea, alertas y *ver en vivo* con Tailscale. El oficial está en [cfia.ysafe.com.br](https://cfia.ysafe.com.br), pero puedes montar el tuyo.

El video **nunca pasa por la nube**: el Agente-Cam-Elo-IA envía solo el resumen (cámaras, máquina, alertas).

## En cualquier VPS Linux

Hostinger, HostGator, Google Cloud, AWS, Oracle, DigitalOcean, Contabo…

1. **Un comando en la VPS** (instala Docker si falta, pregunta el dominio, genera la contraseña de la base y activa HTTPS):

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar-painel.sh | sudo sh
    ```

2. **DNS:** crea un registro **A** de tu dominio apuntando a la IP de la VPS. Los puertos 80 y 443 deben estar libres.
3. **Login con Google:** en Google Cloud, crea un **ID de cliente OAuth** de tipo *Aplicación web* con la redirección `https://TU-DOMINIO/api/auth/google/callback`. Pon el ID y la clave en `/opt/camfacil-painel/.env` y corre `docker compose up -d`.
4. **Apunta los agentes:** en la computadora del cliente, define `CAMFACIL_SITE=https://TU-DOMINIO`.

??? example "Probar en tu PC con Docker"
    ```sh
    mkdir camfacil-painel && cd camfacil-painel
    curl -fsSL https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/deploy/docker-compose.vps.yml -o docker-compose.yml
    printf 'DOMINIO=localhost\nPOSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 20)" > .env
    docker compose up -d
    ```

Imagen lista: `ghcr.io/emersoncassis/camfacil-ia-painel:latest`. Para Dokploy, Coolify o Portainer, usa `deploy/docker-compose.dokploy.yml` ([guía de despliegue](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/DEPLOY.md), en portugués).

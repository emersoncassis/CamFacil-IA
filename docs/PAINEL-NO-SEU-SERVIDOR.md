# Painel no seu servidor

O painel web (login com Google, pareamento dos agentes, lista de clientes) é um contêiner Docker. O oficial roda em **[cfia.ysafe.com.br](https://cfia.ysafe.com.br)**, mas você pode subir o seu em **qualquer VPS Linux** (Hostinger, HostGator, Google Cloud, AWS, Oracle, DigitalOcean, Contabo…) ou no seu PC com Docker. Voltar ao [README](../README.md).

**1. Um comando na VPS** (instala o Docker se faltar, pergunta o domínio, gera a senha do banco e liga HTTPS automático):

```sh
curl -fsSL https://cfia.ysafe.com.br/instalar-painel.sh | sudo sh
```

**2. DNS:** crie um registro **A** do seu domínio (ex.: `painel.suaempresa.com.br`) apontando para o IP da VPS. As portas 80 e 443 precisam estar livres.

**3. Login com Google:** em [console.cloud.google.com](https://console.cloud.google.com) → APIs e serviços → Credenciais → Criar credenciais → **ID do cliente OAuth** → tipo **Aplicativo da Web**. Em *URIs de redirecionamento autorizados* coloque `https://SEU-DOMINIO/api/auth/google/callback`. Copie o ID e a chave secreta para `/opt/camfacil-painel/.env` e rode `cd /opt/camfacil-painel && docker compose up -d`.

**4. Apontar os agentes para o seu painel:** no computador do cliente, abra o programa com a variável `CAMFACIL_SITE=https://SEU-DOMINIO` (Windows: `setx CAMFACIL_SITE https://SEU-DOMINIO` e abra de novo).

<details>
<summary>Testar no seu PC (Docker Desktop) ou instalar à mão</summary>

```sh
mkdir camfacil-painel && cd camfacil-painel
curl -fsSL https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/deploy/docker-compose.vps.yml -o docker-compose.yml
printf 'DOMINIO=localhost\nPOSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 20)" > .env
docker compose up -d     # abre em https://localhost
```

Imagem pronta: `ghcr.io/emersoncassis/camfacil-ia-painel:latest`. Para usar com Dokploy, Coolify ou Portainer, aponte para `deploy/docker-compose.dokploy.yml` (detalhes em [docs/DEPLOY.md](DEPLOY.md)). Atualizar: rode o instalador de novo.

</details>

# Deploy — https://cfia.ysafe.com.br

A API da nuvem (`crates/cfia-api`) roda no Dokploy com um Postgres ao lado. Ela serve o site, o login com Google, o pareamento e a sinalização dos Agentes-Cam-Elo-IA. Vídeo nunca passa por ela ([SDD-08](sdd/SDD-08-contas-e-isolamento.md)).

## 1. DNS
Registro `A` de `cfia.ysafe.com.br` apontando para o IP do servidor do Dokploy.

## 2. Projeto no Dokploy
1. Crie o projeto **CamFácil IA** e, dentro dele, um serviço do tipo **Docker Compose**.
2. Origem: repositório `emersoncassis/CamFacil-IA`, branch `main`, arquivo `deploy/docker-compose.dokploy.yml`.
3. **Environment** (o dono preenche):
   ```
   POSTGRES_PASSWORD=<senha longa e aleatória>
   GOOGLE_CLIENT_ID=<...apps.googleusercontent.com>
   GOOGLE_CLIENT_SECRET=<...>
   ```
4. **Domains:** `cfia.ysafe.com.br` → serviço `cfia-api`, porta `8080`, HTTPS com Let's Encrypt.
5. **Deploy.** Na primeira subida, a API espera o banco e cria as tabelas sozinha.

Sem `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`, o site sobe normalmente e o botão de login avisa que ainda não está configurado.

## Alternativa: pelo terminal (sem o painel)
No servidor do Dokploy, como root:
```sh
curl -fsSL https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/scripts/deploy-servidor.sh | sh
```
O script clona em `/opt/camfacil`, gera a senha do banco em `/opt/camfacil/deploy/.env`, sobe API + Postgres com o Traefik do Dokploy (HTTPS automático) e testa `/api/saude`. Para colocar as chaves do Google, edite o `.env` e rode o script de novo. Esse modo não aparece no painel do Dokploy.

## 3. Google Auth Platform
- Cliente **Aplicativo da Web**.
- Origem autorizada: `https://cfia.ysafe.com.br`.
- Redirecionamento: `https://cfia.ysafe.com.br/api/auth/google/callback`.
- Escopos: `openid`, `email`, `profile`.
- Branding: links `https://cfia.ysafe.com.br/privacidade` e `https://cfia.ysafe.com.br/termos`.
- Publique o app (Audience → Publicar) antes de abrir para os técnicos.

## 4. Conferir
- `https://cfia.ysafe.com.br/api/saude` responde `{"ok":true,...}`.
- O login com Google leva de volta ao painel "Agentes".

## Backup
Ative o backup do volume `cfia-db-dados` no Dokploy (diário). O banco guarda só contas, dispositivos, agentes e auditoria.

## Rodar localmente
```sh
DATABASE_URL="host=/caminho/do/socket port=5432 user=postgres dbname=cfia" \
BASE_URL=http://localhost:8080 cargo run -p cfia-api
```
Testes de integração (precisam de um Postgres):
```sh
TEST_DATABASE_URL="postgres://postgres@localhost/cfia_teste" cargo test -p cfia-api
```

-- CamFácil IA · API da nuvem · esquema inicial (SDD-08)
-- O servidor guarda só quem é o técnico e quais agentes são dele.
-- Nunca: vídeo, senha de câmera, endereço RTSP.

CREATE TABLE IF NOT EXISTS tecnicos (
    id          uuid PRIMARY KEY,
    google_sub  text NOT NULL UNIQUE,
    email       text NOT NULL,
    nome        text NOT NULL DEFAULT '',
    criado_em   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS dispositivos (
    id            uuid PRIMARY KEY,
    tecnico_id    uuid NOT NULL REFERENCES tecnicos(id) ON DELETE CASCADE,
    chave_publica text NOT NULL,
    apelido       text NOT NULL DEFAULT '',
    criado_em     timestamptz NOT NULL DEFAULT now(),
    revogado_em   timestamptz
);
CREATE INDEX IF NOT EXISTS dispositivos_tecnico ON dispositivos(tecnico_id);

CREATE TABLE IF NOT EXISTS agentes (
    id               uuid PRIMARY KEY,
    chave_publica    text NOT NULL UNIQUE,
    dono_id          uuid REFERENCES tecnicos(id) ON DELETE SET NULL,
    apelido_cliente  text NOT NULL DEFAULT '',
    pareado_em       timestamptz,
    ultimo_contato   timestamptz,
    versao           text NOT NULL DEFAULT '',
    criado_em        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS agentes_dono ON agentes(dono_id);

CREATE TABLE IF NOT EXISTS pareamentos (
    codigo_hash     text PRIMARY KEY,
    agente_id       uuid NOT NULL REFERENCES agentes(id) ON DELETE CASCADE,
    tecnico_id      uuid REFERENCES tecnicos(id) ON DELETE CASCADE,
    dispositivo_id  uuid REFERENCES dispositivos(id) ON DELETE CASCADE,
    pedido_id       uuid,
    expira_em       timestamptz NOT NULL,
    usado_em        timestamptz
);

CREATE TABLE IF NOT EXISTS sessoes (
    token_hash      text PRIMARY KEY,
    tecnico_id      uuid NOT NULL REFERENCES tecnicos(id) ON DELETE CASCADE,
    criado_em       timestamptz NOT NULL DEFAULT now(),
    expira_em       timestamptz NOT NULL,
    ip              text NOT NULL DEFAULT '',
    agente_usuario  text NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS sessoes_tecnico ON sessoes(tecnico_id);

CREATE TABLE IF NOT EXISTS oauth_estados (
    estado     text PRIMARY KEY,
    nonce      text NOT NULL,
    verificador text NOT NULL,
    expira_em  timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS auditoria (
    id          bigserial PRIMARY KEY,
    tecnico_id  uuid REFERENCES tecnicos(id) ON DELETE CASCADE,
    tipo        text NOT NULL,
    alvo        text NOT NULL DEFAULT '',
    ip          text NOT NULL DEFAULT '',
    quando      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS auditoria_tecnico ON auditoria(tecnico_id, quando DESC);

-- ---------------------------------------------------------------------------
-- Isolamento no banco (Row-Level Security).
-- A aplicação troca para o papel cfia_app em cada transação e informa o
-- contexto: app.tecnico_id (técnico da sessão) ou app.papel = 'sistema'
-- (operações do próprio servidor, como validar sessão e falar com agentes).
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'cfia_app') THEN
        CREATE ROLE cfia_app NOLOGIN;
    END IF;
END $$;

GRANT USAGE ON SCHEMA public TO cfia_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO cfia_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO cfia_app;

CREATE OR REPLACE FUNCTION cfia_sistema() RETURNS boolean
    LANGUAGE sql STABLE AS $$ SELECT coalesce(current_setting('app.papel', true), '') = 'sistema' $$;

CREATE OR REPLACE FUNCTION cfia_tecnico() RETURNS uuid
    LANGUAGE sql STABLE AS $$ SELECT nullif(current_setting('app.tecnico_id', true), '')::uuid $$;

ALTER TABLE tecnicos      ENABLE ROW LEVEL SECURITY;
ALTER TABLE dispositivos  ENABLE ROW LEVEL SECURITY;
ALTER TABLE agentes       ENABLE ROW LEVEL SECURITY;
ALTER TABLE pareamentos   ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessoes       ENABLE ROW LEVEL SECURITY;
ALTER TABLE oauth_estados ENABLE ROW LEVEL SECURITY;
ALTER TABLE auditoria     ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS isolamento ON tecnicos;
CREATE POLICY isolamento ON tecnicos USING (cfia_sistema() OR id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR id = cfia_tecnico());
DROP POLICY IF EXISTS isolamento ON dispositivos;
CREATE POLICY isolamento ON dispositivos USING (cfia_sistema() OR tecnico_id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR tecnico_id = cfia_tecnico());
DROP POLICY IF EXISTS isolamento ON agentes;
CREATE POLICY isolamento ON agentes USING (cfia_sistema() OR dono_id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR dono_id = cfia_tecnico());
DROP POLICY IF EXISTS isolamento ON pareamentos;
CREATE POLICY isolamento ON pareamentos USING (cfia_sistema()) WITH CHECK (cfia_sistema());
DROP POLICY IF EXISTS isolamento ON sessoes;
CREATE POLICY isolamento ON sessoes USING (cfia_sistema() OR tecnico_id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR tecnico_id = cfia_tecnico());
DROP POLICY IF EXISTS isolamento ON oauth_estados;
CREATE POLICY isolamento ON oauth_estados USING (cfia_sistema()) WITH CHECK (cfia_sistema());
DROP POLICY IF EXISTS isolamento ON auditoria;
CREATE POLICY isolamento ON auditoria USING (cfia_sistema() OR tecnico_id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR tecnico_id = cfia_tecnico());

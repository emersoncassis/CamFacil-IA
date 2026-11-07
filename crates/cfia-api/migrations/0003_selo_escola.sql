-- Selo da Escolinha de IA: um por técnico, com código público para compartilhar.
-- Para onde voltar depois do login (ex.: /aprender), validado no servidor.
ALTER TABLE oauth_estados ADD COLUMN IF NOT EXISTS voltar text;

CREATE TABLE IF NOT EXISTS selos_escola (
    codigo          text PRIMARY KEY,
    tecnico_id      uuid NOT NULL UNIQUE REFERENCES tecnicos(id) ON DELETE CASCADE,
    nome            text NOT NULL,
    instagram       text,
    autoriza_repost boolean NOT NULL DEFAULT false,
    criado_em       timestamptz NOT NULL DEFAULT now()
);
GRANT SELECT, INSERT, UPDATE, DELETE ON selos_escola TO cfia_app;
ALTER TABLE selos_escola ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS isolamento ON selos_escola;
CREATE POLICY isolamento ON selos_escola USING (cfia_sistema() OR tecnico_id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR tecnico_id = cfia_tecnico());

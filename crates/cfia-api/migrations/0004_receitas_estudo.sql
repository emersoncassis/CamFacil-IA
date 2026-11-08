-- Modo estúdio (SDD-07, ADR 0013): receitas compartilhadas entre técnicos.
-- Só a receita (JSON validado no servidor) e as métricas do treino.
-- Nunca: fotos, rótulos, modelo ONNX, nome ou endereço do cliente.

CREATE TABLE IF NOT EXISTS receitas_estudo (
    id              uuid PRIMARY KEY,
    tecnico_id      uuid NOT NULL REFERENCES tecnicos(id) ON DELETE CASCADE,
    slug            text NOT NULL,
    nome            text NOT NULL,
    descricao       text NOT NULL DEFAULT '',
    classes         text[] NOT NULL,
    receita         jsonb NOT NULL,
    receita_sha256  text NOT NULL,
    metricas        jsonb,
    autor           text NOT NULL DEFAULT '',
    publica         boolean NOT NULL DEFAULT true,
    downloads       integer NOT NULL DEFAULT 0,
    criado_em       timestamptz NOT NULL DEFAULT now(),
    atualizado_em   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tecnico_id, slug),
    CHECK (octet_length(receita::text) <= 65536)
);
CREATE INDEX IF NOT EXISTS receitas_estudo_publicas ON receitas_estudo(atualizado_em DESC) WHERE publica;

GRANT SELECT, INSERT, UPDATE, DELETE ON receitas_estudo TO cfia_app;
ALTER TABLE receitas_estudo ENABLE ROW LEVEL SECURITY;

-- Leitura: a receita pública é de todos; a não pública, só do dono.
-- Escrita: só o dono (ou o sistema, que conta downloads).
DROP POLICY IF EXISTS leitura ON receitas_estudo;
CREATE POLICY leitura ON receitas_estudo FOR SELECT
    USING (publica OR cfia_sistema() OR tecnico_id = cfia_tecnico());
DROP POLICY IF EXISTS insercao ON receitas_estudo;
CREATE POLICY insercao ON receitas_estudo FOR INSERT
    WITH CHECK (cfia_sistema() OR tecnico_id = cfia_tecnico());
DROP POLICY IF EXISTS alteracao ON receitas_estudo;
CREATE POLICY alteracao ON receitas_estudo FOR UPDATE
    USING (cfia_sistema() OR tecnico_id = cfia_tecnico())
    WITH CHECK (cfia_sistema() OR tecnico_id = cfia_tecnico());
DROP POLICY IF EXISTS remocao ON receitas_estudo;
CREATE POLICY remocao ON receitas_estudo FOR DELETE
    USING (cfia_sistema() OR tecnico_id = cfia_tecnico());

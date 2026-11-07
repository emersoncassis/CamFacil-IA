-- Último resumo que o agente mandou (sem imagem, sem senha, sem endereço RTSP):
-- câmeras com nome e situação, uso da máquina, versão e alertas recentes.
ALTER TABLE agentes ADD COLUMN IF NOT EXISTS estado    jsonb;
ALTER TABLE agentes ADD COLUMN IF NOT EXISTS estado_em timestamptz;

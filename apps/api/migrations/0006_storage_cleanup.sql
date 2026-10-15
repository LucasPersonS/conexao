-- Limpeza do storage (src/cleanup.rs).
-- Quando o arquivo foi enviado ou consultado pela última vez: a limpeza só apaga o que
-- nenhuma versão usa e ninguém tocou nas últimas 48 h (um envio em andamento fica a salvo).
ALTER TABLE stored_files ADD COLUMN last_used_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- Quando a equipe mudou a aprovação: versões de servidores recusados há mais de 7 dias são apagadas.
ALTER TABLE servers ADD COLUMN approval_updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

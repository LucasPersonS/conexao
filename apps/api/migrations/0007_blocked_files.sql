-- Arquivos derrubados pela equipe (painel → Armazenamento): saem do disco na hora e
-- não podem ser enviados de novo.
CREATE TABLE blocked_files (
    sha1        TEXT PRIMARY KEY,
    size        BIGINT NOT NULL DEFAULT 0,
    path        TEXT NOT NULL DEFAULT '',
    reason      TEXT NOT NULL DEFAULT '',
    blocked_by  UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

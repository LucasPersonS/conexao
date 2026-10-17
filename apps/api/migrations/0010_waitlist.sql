-- Lista de espera do launcher (acesso antecipado). Com CONEXAO_WAITLIST ligado, só entra no
-- launcher quem tem launcher_access, é da equipe Conexão ou da equipe de um servidor parceiro.

ALTER TABLE users ADD COLUMN launcher_access BOOLEAN NOT NULL DEFAULT FALSE;

-- Quem já tinha conta antes da lista continua com acesso.
UPDATE users SET launcher_access = TRUE;

-- Pedidos feitos pelo site (login pelo Discord + nick). discord_id NULL = liberado à mão
-- pela equipe pelo nick (conta original).
CREATE TABLE waitlist (
    id              UUID PRIMARY KEY,
    discord_id      TEXT UNIQUE,
    discord_name    TEXT NOT NULL DEFAULT '',
    discord_avatar  TEXT,
    minecraft_name  TEXT NOT NULL,
    -- Só conta original (conferida na Mojang); a não original é reconhecida pelo Discord.
    minecraft_uuid  UUID,
    account_type    TEXT NOT NULL CHECK (account_type IN ('microsoft', 'offline')),
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    note            TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at      TIMESTAMPTZ,
    decided_by      UUID REFERENCES users (id) ON DELETE SET NULL
);
CREATE INDEX waitlist_status ON waitlist (status, created_at);
CREATE INDEX waitlist_minecraft_uuid ON waitlist (minecraft_uuid) WHERE minecraft_uuid IS NOT NULL;

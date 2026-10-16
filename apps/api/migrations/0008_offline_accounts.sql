-- Contas não originais (login pelo Discord) e servidores em online-mode=false.

-- microsoft: conta Minecraft original verificada na Mojang.
-- offline: conta criada pelo Discord; o UUID é o UUID offline do nick.
ALTER TABLE users
    ADD COLUMN account_type TEXT NOT NULL DEFAULT 'microsoft'
        CHECK (account_type IN ('microsoft', 'offline'));

-- Nick de conta não original é único (sem diferenciar maiúsculas).
CREATE UNIQUE INDEX users_offline_name_unique ON users (lower(minecraft_name)) WHERE account_type = 'offline';
CREATE INDEX users_name_lower ON users (lower(minecraft_name));

-- Sessões longas do launcher para contas não originais (o JWT dura 12 h; esta renova).
CREATE TABLE offline_sessions (
    token_sha256  TEXT PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX offline_sessions_user ON offline_sessions (user_id);

-- O dono escolhe se o servidor aceita contas não originais (exige online-mode=false + Companion).
ALTER TABLE servers ADD COLUMN allow_offline BOOLEAN NOT NULL DEFAULT FALSE;

-- Passe de entrada: o launcher pede antes de abrir o jogo; o Companion só deixa entrar
-- quem tem passe para aquele nick vindo daquele IP.
CREATE TABLE join_tickets (
    id           UUID PRIMARY KEY,
    server_id    UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    player_name  TEXT NOT NULL,
    ip           TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX join_tickets_server ON join_tickets (server_id, expires_at);

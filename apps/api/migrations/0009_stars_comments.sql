-- Estrelas e comentários dos jogadores nos servidores (launcher).

-- Uma estrela por jogador por servidor; o total aparece nos cards e na página do servidor.
CREATE TABLE server_stars (
    server_id   UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (server_id, user_id)
);
CREATE INDEX server_stars_user ON server_stars (user_id);

-- Comentários: só quem tem whitelist aprovada no servidor ou conta cadastrada pelo Discord.
-- A equipe do servidor (moderador ou acima) apaga pelo painel; o autor apaga o próprio.
CREATE TABLE server_comments (
    id          UUID PRIMARY KEY,
    server_id   UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    author_id   UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    body        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX server_comments_server ON server_comments (server_id, created_at DESC);
CREATE INDEX server_comments_author ON server_comments (author_id, created_at DESC);

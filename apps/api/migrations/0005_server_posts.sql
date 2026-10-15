-- Notícias e eventos publicados pela equipe de cada servidor (aparecem no launcher).
CREATE TABLE server_posts (
    id          UUID PRIMARY KEY,
    server_id   UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('news', 'event')),
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    starts_at   TIMESTAMPTZ,
    author_id   UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kind = 'news' OR starts_at IS NOT NULL)
);
CREATE INDEX server_posts_server ON server_posts (server_id, created_at DESC);
CREATE INDEX server_posts_events ON server_posts (starts_at) WHERE kind = 'event';

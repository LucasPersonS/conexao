-- CENTRAL · esquema inicial

CREATE TABLE users (
    id              UUID PRIMARY KEY,
    minecraft_uuid  UUID NOT NULL UNIQUE,
    minecraft_name  TEXT NOT NULL,
    discord_id      TEXT UNIQUE,
    is_staff        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE servers (
    id                  UUID PRIMARY KEY,
    slug                TEXT NOT NULL UNIQUE,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    address             TEXT NOT NULL,
    minecraft_version   TEXT NOT NULL,
    loader              TEXT NOT NULL CHECK (loader IN ('fabric', 'forge', 'neoforge')),
    loader_version      TEXT NOT NULL,
    tags                TEXT[] NOT NULL DEFAULT '{}',
    icon_url            TEXT,
    banner_url          TEXT,
    discord_url         TEXT,
    whitelist_required  BOOLEAN NOT NULL DEFAULT TRUE,
    -- pending: aguardando aprovação da equipe CENTRAL; approved: aparece na home
    approval            TEXT NOT NULL DEFAULT 'pending' CHECK (approval IN ('pending', 'approved', 'rejected', 'suspended')),
    -- destaque pago na home
    featured_until      TIMESTAMPTZ,
    status_online       BOOLEAN NOT NULL DEFAULT FALSE,
    status_players      INTEGER NOT NULL DEFAULT 0,
    status_max_players  INTEGER NOT NULL DEFAULT 0,
    status_latency_ms   INTEGER,
    status_checked_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE server_members (
    server_id   UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'moderator')),
    PRIMARY KEY (server_id, user_id)
);

CREATE TABLE pack_versions (
    id               UUID PRIMARY KEY,
    server_id        UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    version          TEXT NOT NULL,
    manifest         JSONB NOT NULL,
    manifest_sha256  TEXT NOT NULL,
    published_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    published_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (server_id, version)
);

-- Ponteiro do canal para a versão ativa (rollback = mover o ponteiro).
CREATE TABLE server_channels (
    server_id   UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    channel     TEXT NOT NULL CHECK (channel IN ('stable', 'beta')),
    version_id  UUID NOT NULL REFERENCES pack_versions(id) ON DELETE CASCADE,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (server_id, channel)
);

CREATE TABLE stored_files (
    sha1        TEXT PRIMARY KEY,
    size        BIGINT NOT NULL,
    uploaded_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE whitelist_entries (
    server_id    UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status       TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'banned')),
    note         TEXT,
    decided_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (server_id, user_id)
);

CREATE TABLE agents (
    server_id     UUID PRIMARY KEY REFERENCES servers(id) ON DELETE CASCADE,
    token_sha256  TEXT NOT NULL UNIQUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ,
    agent_version TEXT
);

CREATE TABLE audit_log (
    id          BIGSERIAL PRIMARY KEY,
    server_id   UUID REFERENCES servers(id) ON DELETE CASCADE,
    actor_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,
    details     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audit_log_server_idx ON audit_log (server_id, created_at DESC);
CREATE INDEX whitelist_status_idx ON whitelist_entries (server_id, status);

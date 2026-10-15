-- Cache das consultas ao Modrinth por SHA-1 (mods reconhecidos viram links,
-- sem re-hospedar no CENTRAL). `found = false` também é guardado, por menos tempo.
CREATE TABLE modrinth_files (
    sha1        TEXT PRIMARY KEY,
    found       BOOLEAN NOT NULL,
    data        JSONB,
    checked_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Vínculo do CENTRAL Companion (mod do servidor) com um servidor do painel.
-- O mod gera um segredo, mostra o código no console e o dono confirma no painel.
CREATE TABLE companion_links (
    code            TEXT PRIMARY KEY,
    secret_sha256   TEXT NOT NULL UNIQUE,
    info            JSONB NOT NULL,
    server_id       UUID REFERENCES servers(id) ON DELETE CASCADE,
    -- token do agente entregue ao mod uma única vez (apagado após a entrega)
    agent_token     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    linked_at       TIMESTAMPTZ
);

-- Última fotografia do servidor enviada pelo Companion (versões e mods instalados).
CREATE TABLE server_snapshots (
    server_id    UUID PRIMARY KEY REFERENCES servers(id) ON DELETE CASCADE,
    info         JSONB NOT NULL DEFAULT '{}',
    mods         JSONB NOT NULL DEFAULT '[]',
    reported_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Agentes autenticam uploads pedidos pela API (arquivos que não estão no Modrinth).
ALTER TABLE stored_files ADD COLUMN uploaded_by_server UUID REFERENCES servers(id) ON DELETE SET NULL;

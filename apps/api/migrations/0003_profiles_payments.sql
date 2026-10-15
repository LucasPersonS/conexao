-- Perfis personalizados dos jogadores.
CREATE TABLE user_profiles (
    user_id             UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    display_name        TEXT,
    pronouns            TEXT,
    bio                 TEXT,
    links               JSONB NOT NULL DEFAULT '{}',
    theme               JSONB NOT NULL DEFAULT '{}',
    -- quem vê o Discord: everyone | staff (equipe dos servidores onde ele está) | nobody
    discord_visibility  TEXT NOT NULL DEFAULT 'everyone' CHECK (discord_visibility IN ('everyone', 'staff', 'nobody')),
    -- fundo enviado: só aparece para os outros depois de aprovado pela equipe CENTRAL
    background_url      TEXT,
    background_status   TEXT CHECK (background_status IN ('pending', 'approved', 'rejected')),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Itens premium (molduras animadas, fontes, efeitos). Enquanto
-- CENTRAL_PREMIUM_ENFORCED=false, todos podem usar.
ALTER TABLE users ADD COLUMN premium_until TIMESTAMPTZ;

CREATE TABLE profile_reports (
    id           UUID PRIMARY KEY,
    target_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reporter_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason       TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'dismissed', 'actioned')),
    resolved_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at  TIMESTAMPTZ
);
-- Uma denúncia aberta por pessoa e perfil.
CREATE UNIQUE INDEX profile_reports_open ON profile_reports (target_id, reporter_id) WHERE status = 'open';

-- Compras de destaque (Mercado Pago Checkout Pro).
CREATE TABLE featured_purchases (
    id               UUID PRIMARY KEY,
    server_id        UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    user_id          UUID REFERENCES users(id) ON DELETE SET NULL,
    days             INTEGER NOT NULL CHECK (days > 0),
    amount_cents     INTEGER NOT NULL CHECK (amount_cents > 0),
    status           TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled', 'refunded')),
    mp_preference_id TEXT,
    mp_payment_id    TEXT UNIQUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at          TIMESTAMPTZ,
    -- até quando o destaque foi estendido por esta compra
    featured_until   TIMESTAMPTZ
);
CREATE INDEX featured_purchases_server ON featured_purchases (server_id, created_at DESC);

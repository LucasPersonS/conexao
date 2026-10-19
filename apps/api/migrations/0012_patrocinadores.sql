-- Banners de patrocinadores no launcher (home, página do servidor e tela de carregamento).
-- Cadastrados só pela equipe Conexão. Visualizações e cliques contados por dia, sem duplicar
-- a mesma pessoa no mesmo dia (para o relatório ao patrocinador ser confiável).

CREATE TABLE sponsors (
    id              UUID PRIMARY KEY,
    name            TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    -- Banner largo (8:1), usado na home e na tela de carregamento.
    image_url       TEXT NOT NULL,
    -- Opcional: versão 2:1 para a lateral da página do servidor (sem ela, usa o banner largo).
    image_side_url  TEXT,
    link_url        TEXT NOT NULL CHECK (link_url LIKE 'https://%' AND char_length(link_url) <= 500),
    -- Texto curto para acessibilidade e para quando a imagem não carrega.
    alt_text        TEXT CHECK (alt_text IS NULL OR char_length(alt_text) <= 140),
    placements      TEXT[] NOT NULL
                    CHECK (cardinality(placements) > 0 AND placements <@ ARRAY['home', 'server', 'loading']::TEXT[]),
    -- Peso no sorteio da rotação (1 a 10): quem paga mais aparece mais vezes.
    weight          INTEGER NOT NULL DEFAULT 1 CHECK (weight BETWEEN 1 AND 10),
    starts_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    ends_at         TIMESTAMPTZ,
    active          BOOLEAN NOT NULL DEFAULT TRUE,
    created_by      UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (ends_at IS NULL OR ends_at > starts_at)
);
CREATE INDEX sponsors_live ON sponsors (active, starts_at, ends_at);

-- Totais por dia e posição.
CREATE TABLE sponsor_stats (
    sponsor_id  UUID NOT NULL REFERENCES sponsors (id) ON DELETE CASCADE,
    day         DATE NOT NULL,
    placement   TEXT NOT NULL CHECK (placement IN ('home', 'server', 'loading')),
    views       BIGINT NOT NULL DEFAULT 0,
    clicks      BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (sponsor_id, day, placement)
);

-- Quem já contou hoje (uma visualização e um clique por pessoa, dia e posição).
-- Guardado só por 2 dias; depois sobram apenas os totais.
CREATE TABLE sponsor_seen (
    sponsor_id  UUID NOT NULL REFERENCES sponsors (id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day         DATE NOT NULL,
    placement   TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('view', 'click')),
    PRIMARY KEY (sponsor_id, user_id, day, placement, kind)
);
CREATE INDEX sponsor_seen_day ON sponsor_seen (day);

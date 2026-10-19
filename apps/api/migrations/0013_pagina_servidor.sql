-- Página personalizada do servidor (PRO) e link de formulário para pedir whitelist.

-- Link do Google Forms mostrado para quem ainda não tem whitelist (todos os servidores).
ALTER TABLE servers ADD COLUMN whitelist_form_url TEXT
    CHECK (whitelist_form_url IS NULL OR (whitelist_form_url LIKE 'https://%' AND char_length(whitelist_form_url) <= 500));

-- Tema (gradiente, cor de destaque) e blocos (texto, imagem/GIF, galeria, vídeo do YouTube).
-- Só aparece no launcher com o PRO ativo; quando o PRO vence, fica guardada e volta ao renovar.
-- Formato validado pela API (apps/api/src/routes/page.rs): nada de HTML/CSS livre.
ALTER TABLE servers ADD COLUMN page JSONB NOT NULL DEFAULT '{}'::JSONB;
ALTER TABLE servers ADD COLUMN page_updated_at TIMESTAMPTZ;

-- Imagens e GIFs enviados para a página: limite de espaço por servidor e envios por dia.
-- O arquivo em si fica em {storage}/images/{sha1}.{ext} (o mesmo do ícone e do banner).
CREATE TABLE server_page_media (
    id          UUID PRIMARY KEY,
    server_id   UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('image', 'gif')),
    bytes       BIGINT NOT NULL CHECK (bytes >= 0),
    uploaded_by UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (server_id, url)
);
CREATE INDEX server_page_media_recent ON server_page_media (server_id, created_at);

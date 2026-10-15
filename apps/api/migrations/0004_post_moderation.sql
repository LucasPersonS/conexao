-- Moderação depois de publicar: fundos aparecem na hora e a equipe remove o que
-- for impróprio. 'pending' passa a significar "ainda não visto pela equipe".

-- Perfil bloqueado: o jogador continua jogando, mas não personaliza mais o perfil.
ALTER TABLE users ADD COLUMN profile_banned_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN profile_ban_reason TEXT;

-- Imagens removidas pela moderação não podem ser enviadas de novo (nome = SHA-1).
CREATE TABLE blocked_images (
    url         TEXT PRIMARY KEY,
    blocked_by  UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

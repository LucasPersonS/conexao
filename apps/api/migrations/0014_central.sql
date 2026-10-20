-- Parceria com a Central SMP's (comunidade no Discord).
--
-- Origem: quem entra na lista de espera pelo link `?ref=central` fica com `ref = 'central'`.
-- Membro verificado: o login do Discord (escopo `guilds`) confirmou que a pessoa está no
-- servidor da Central (`CONEXAO_CENTRAL_GUILD_ID`). Só o membro verificado ganha o selo.
ALTER TABLE waitlist ADD COLUMN ref TEXT CHECK (ref IS NULL OR ref ~ '^[a-z0-9-]{1,32}$');
ALTER TABLE waitlist ADD COLUMN central_member BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX waitlist_central ON waitlist (created_at, id) WHERE status = 'pending' AND central_member;

-- Selo "Membro da Central" no perfil do jogador.
ALTER TABLE users ADD COLUMN central_member BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN signup_ref TEXT CHECK (signup_ref IS NULL OR signup_ref ~ '^[a-z0-9-]{1,32}$');

-- Programa "SMP da Central": selo no card do servidor e no perfil da equipe dele.
ALTER TABLE servers ADD COLUMN central_partner BOOLEAN NOT NULL DEFAULT FALSE;

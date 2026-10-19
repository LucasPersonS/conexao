-- Monetização: Moedas Conexão, Conexão+ e PRO pré-pagos e apoio a servidores (split Mercado Pago).
--
-- Regras que o banco garante sozinho (mesmo se o código tiver um bug):
--   * cada crédito/débito de moedas tem uma referência ÚNICA → nunca credita duas vezes;
--   * o saldo fica guardado junto do histórico, na mesma transação;
--   * pedidos só andam por estados válidos e um pagamento do Mercado Pago só vale para um pedido.

-- ------------------------------------------------------------ Moedas Conexão
-- Circuito fechado: compra com dinheiro, gasta só dentro do Conexão. Sem saque e sem
-- transferência entre jogadores. O saldo só fica negativo quando um pagamento de moedas
-- já gastas é estornado (vira dívida e bloqueia novos gastos até zerar).
ALTER TABLE users ADD COLUMN coin_balance BIGINT NOT NULL DEFAULT 0;

CREATE TABLE coin_ledger (
    id             UUID PRIMARY KEY,
    user_id        UUID NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    delta          BIGINT NOT NULL CHECK (delta <> 0),
    balance_after  BIGINT NOT NULL,
    kind           TEXT NOT NULL CHECK (kind IN ('purchase', 'spend', 'refund', 'chargeback', 'grant', 'revoke')),
    -- Referência única da operação (ex.: order:<id>, spend:<user>:<chave>). Impede duplicar.
    ref            TEXT NOT NULL UNIQUE,
    description    TEXT NOT NULL,
    -- Equipe que deu/tirou moedas manualmente.
    created_by     UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX coin_ledger_user ON coin_ledger (user_id, created_at DESC);

-- ------------------------------------------------------------ PRO do servidor
ALTER TABLE servers ADD COLUMN pro_until TIMESTAMPTZ;

-- ------------------------------------------------------------ Pedidos
-- Um pedido por compra: pacote de moedas, Conexão+, PRO, destaque pago com moedas ou apoio.
CREATE TABLE orders (
    id                 UUID PRIMARY KEY,
    user_id            UUID REFERENCES users (id) ON DELETE SET NULL,
    kind               TEXT NOT NULL CHECK (kind IN ('coins', 'plus', 'pro', 'featured', 'support')),
    sku                TEXT NOT NULL,
    server_id          UUID REFERENCES servers (id) ON DELETE SET NULL,
    pay_with           TEXT NOT NULL CHECK (pay_with IN ('money', 'coins')),
    -- Valor em centavos (dinheiro) — sempre calculado no servidor, nunca vindo do cliente.
    amount_cents       INTEGER NOT NULL CHECK (amount_cents >= 0),
    -- Moedas creditadas (pacote) ou gastas (pagamento com moedas).
    coins              BIGINT NOT NULL DEFAULT 0 CHECK (coins >= 0),
    days               INTEGER CHECK (days IS NULL OR days > 0),
    -- Apoio: taxa do Conexão (centavos) e conta Mercado Pago do servidor que recebe.
    fee_cents          INTEGER NOT NULL DEFAULT 0 CHECK (fee_cents >= 0 AND fee_cents <= amount_cents),
    seller_mp_user_id  TEXT,
    message            TEXT CHECK (message IS NULL OR char_length(message) <= 140),
    anonymous          BOOLEAN NOT NULL DEFAULT FALSE,
    status             TEXT NOT NULL DEFAULT 'pending'
                       CHECK (status IN ('pending', 'approved', 'rejected', 'cancelled', 'refunded')),
    mp_preference_id   TEXT,
    mp_payment_id      TEXT UNIQUE,
    -- Até quando o benefício foi estendido por este pedido (Conexão+, PRO, destaque).
    granted_until      TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at            TIMESTAMPTZ,
    refunded_at        TIMESTAMPTZ,
    -- Última consulta ao Mercado Pago por conciliação (limita consultas repetidas).
    last_checked_at    TIMESTAMPTZ,
    CHECK (kind <> 'support' OR (server_id IS NOT NULL AND pay_with = 'money' AND seller_mp_user_id IS NOT NULL)),
    CHECK (kind <> 'coins' OR pay_with = 'money'),
    CHECK (pay_with <> 'coins' OR amount_cents = 0)
);
CREATE INDEX orders_user ON orders (user_id, created_at DESC);
CREATE INDEX orders_server ON orders (server_id, created_at DESC) WHERE server_id IS NOT NULL;
CREATE INDEX orders_pending ON orders (user_id, created_at) WHERE status = 'pending';

-- ------------------------------------------------------------ Conta Mercado Pago dos servidores
-- Tokens OAuth guardados CRIPTOGRAFADOS (AES-256-GCM, chave fora do banco: CONEXAO_SECRETS_KEY).
-- Um dump do banco sozinho não dá acesso às contas dos donos.
CREATE TABLE server_payout_accounts (
    server_id          UUID PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    mp_user_id         TEXT NOT NULL,
    access_token_enc   BYTEA NOT NULL,
    refresh_token_enc  BYTEA,
    token_expires_at   TIMESTAMPTZ NOT NULL,
    live_mode          BOOLEAN NOT NULL DEFAULT TRUE,
    support_enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    connected_by       UUID REFERENCES users (id) ON DELETE SET NULL,
    connected_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Renovação do token em andamento/falha recente: uma por vez, no máximo a cada 10 min.
    refresh_attempt_at TIMESTAMPTZ
);

-- Pedidos de conexão em andamento (OAuth). O `state` fica só como hash; uso único; 10 min.
CREATE TABLE mp_oauth_states (
    state_sha256   TEXT PRIMARY KEY,
    server_id      UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    user_id        UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_verifier  TEXT NOT NULL,
    expires_at     TIMESTAMPTZ NOT NULL
);

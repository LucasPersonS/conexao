# Monetização do Conexão

Quatro fontes de receita, todas pelo Mercado Pago (Pix, cartão e boleto):

| Produto | Quem paga | Como paga | O que entrega |
|---|---|---|---|
| **Moedas Conexão** | Jogador | Dinheiro | Saldo para gastar dentro do Conexão |
| **Conexão+** | Jogador | Dinheiro ou Moedas | Molduras, fontes e efeitos premium + selo "Conexão+" |
| **PRO** | Admin/dono do servidor | Dinheiro ou Moedas | Mais espaço, selo PRO no launcher, estatísticas de 30 dias |
| **Apoio a servidor** | Jogador | Só dinheiro | Dinheiro vai para o dono; o Conexão fica com 15% |

O destaque pago continua como antes (painel → Visão geral) e agora também aceita Moedas.

## Moedas

- 1 moeda = R$ 0,01 nos preços em moedas (Conexão+ de R$ 9,90 = 990 moedas).
- Pacotes maiores dão bônus (padrão: 500 / 1.050 / 2.200 / 5.750 moedas por R$ 5 / 10 / 20 / 50).
- **Circuito fechado:** moedas não são sacadas, não vão para outra pessoa e não pagam apoio.
  Isso evita lavagem de dinheiro, mercado paralelo de contas e regras do Banco Central para
  arranjos de pagamento.
- A equipe dá ou tira moedas (ex.: crédito de booster) com `POST /v1/staff/coins`
  `{ "name": "<nick>", "delta": 500, "reason": "Booster 2x" }`. Precisa ser uma conta da
  equipe (o token de equipe não vale), tudo fica no histórico e no log de auditoria.

## Apoio a servidores (split)

1. O **dono** do servidor abre o painel → **PRO e apoios** → *Conectar conta Mercado Pago*.
   Só o dono (nem admin, nem a equipe Conexão) conecta, troca ou desconecta essa conta.
2. O jogador clica em **Apoiar** na página do servidor no launcher (R$ 5 a R$ 500).
3. O link de pagamento é criado com o token do dono e `marketplace_fee` = 15%. O Mercado Pago
   credita a parte do dono direto na conta dele e a taxa na conta do Conexão. **O Conexão nunca
   guarda o dinheiro dos servidores.** A taxa do Mercado Pago sai da parte do dono.
4. O mural público mostra quem apoiou (ou "anônimo") e a mensagem. Valores só aparecem para a
   equipe do servidor.

## Configuração (VPS)

1. `conexao-api gerar-chave-segredos` → `CONEXAO_SECRETS_KEY` no `.env`. **Guarde um backup fora
   da VPS.** Sem a chave, todos os donos precisam reconectar a conta.
2. No Mercado Pago Developers, no app do Conexão:
   - copie *Client ID* e *Client Secret* para `CONEXAO_MP_CLIENT_ID` / `CONEXAO_MP_CLIENT_SECRET`;
   - cadastre a URL de redirecionamento `https://<CONEXAO_DOMAIN>/v1/payments/mercadopago/oauth/callback`;
   - deixe o PKCE ligado (o Conexão já manda `code_challenge` S256).
3. Para **vender o Conexão+**, ligue `CONEXAO_PREMIUM_ENFORCED=true`. Enquanto estiver `false`,
   os itens premium continuam grátis para todos.
4. Preços e limites: `CONEXAO_COIN_PACKS`, `CONEXAO_PLUS_PLANS`, `CONEXAO_PRO_PLANS`,
   `CONEXAO_PRO_QUOTA_MB`, `CONEXAO_SUPPORT_FEE_PERCENT`, `CONEXAO_SUPPORT_MIN`, `CONEXAO_SUPPORT_MAX`
   (veja `deploy/.env.example`).
5. Recrie a API (`docker compose up -d api`). A migração `0011_monetizacao.sql` roda sozinha.

Em produção a API não sobe se faltar `CONEXAO_SECRETS_KEY`, o webhook secret ou se só uma das
credenciais do app estiver preenchida.

## Segurança: o que foi pensado

**Preço e entrega**
- O cliente só manda o código do item (`sku`). Valor, dias e moedas vêm da configuração do servidor.
- O webhook só diz "olhe o pagamento X". A API sempre busca o pagamento no Mercado Pago e confere:
  referência do pedido, valor ≥ preço, moeda BRL e, no apoio, se quem recebeu foi a conta do dono.
- Avisos de apoio precisam da assinatura do Mercado Pago (`x-signature`). Sem ela a API nem consulta.

**Nada é entregue duas vezes**
- Cada pedido é travado (`SELECT … FOR UPDATE`) e só anda por estados válidos.
- `mp_payment_id` é único. Cada movimento de moedas tem uma referência única no banco.
- Gasto com moedas: o débito só passa se houver saldo, na mesma instrução SQL, dentro da
  transação que entrega o item. Cada clique leva uma chave única: clique duplo não cobra duas vezes.
  Testado com 12 compras simultâneas: só 1 passa.

**Estornos e contestações**
- Estorno desfaz o que foi entregue (dias de Conexão+/PRO/destaque, moedas).
- Contestação de moedas que já foram gastas: desfaz as compras feitas com elas (PRO, destaque,
  Conexão+), da mais recente para a mais antiga, até o saldo voltar a zero. Quem compra moedas
  com cartão roubado, gasta e contesta não fica com nada.
- Estorno que chega antes da aprovação fecha o pedido (um "aprovado" atrasado não entrega).
- Estornar um pagamento duplicado não tira o benefício pago pelo outro.

**Contas dos donos (OAuth)**
- `state` aleatório (256 bits), guardado só como hash, uso único, 10 minutos, preso ao usuário e ao
  servidor. PKCE S256 junto.
- Tokens criptografados com AES-256-GCM (chave fora do banco). O id do servidor entra como dado
  associado: copiar o token de uma linha para outra não funciona. Um dump do banco sozinho não dá
  acesso a nenhuma conta.
- Tokens nunca voltam para o painel. O painel mostra só os 4 últimos dígitos da conta.
- Renovação do token: uma por vez por servidor, no máximo a cada 10 minutos.
- Em produção, conta de teste do Mercado Pago não é aceita (evita apoios falsos).

**Abuso**
- Limites por IP: compras (10/min), conexão de conta (2/min), avisos do Mercado Pago (120/min).
- No máximo 5 pagamentos em aberto por conta (pedidos esquecidos expiram em 24 h).
- Consulta ao Mercado Pago por pedido limitada a 1 a cada 20 s.
- Mensagens de apoio: até 140 caracteres, sem links, com o filtro de palavras do Conexão.
- O launcher só chama as rotas da loja que estão na lista do `route_allowed`. Rotas da equipe e de
  conexão de conta ficam de fora. Os links de pagamento só abrem se forem do Mercado Pago.

## Testes

- `cargo test -p conexao-api`: unidade (catálogo, taxa, PKCE, criptografia, mensagens).
- Com um PostgreSQL de teste: corridas, idempotência, estornos, contestação e split.
  ```
  CONEXAO_TEST_DATABASE_URL=postgres://postgres@localhost:5432/conexao_test cargo test -p conexao-api
  ```

## Antes de abrir para todos

- [ ] Fazer uma compra real de cada tipo com valor baixo (Pix) e conferir o extrato.
- [ ] Conectar uma conta real de dono, apoiar com R$ 5 e conferir o split no extrato dos dois lados.
- [ ] Confirmar no Mercado Pago que os avisos de apoio chegam assinados. Se não chegarem, o launcher
      ainda confirma sozinho ao consultar o pedido.
- [ ] Revisar com um contador como declarar a receita (taxa de apoio = receita de intermediação).

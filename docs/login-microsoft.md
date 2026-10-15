# Login Microsoft do Conexão — situação e como ter o próprio

## Como funciona hoje

O launcher usa o mesmo fluxo de login do **Modrinth App** (via `theseus`): o
"Xbox SISU" com a identidade do launcher oficial do Minecraft. Funciona para
qualquer conta Microsoft com Minecraft Java e não exige cadastro nosso — por isso
os testes já funcionam.

O risco: depender de uma identidade que não é do Conexão (a Microsoft pode
restringir) e a tela de login mostrar "Minecraft" em vez de "Conexão".

## Para ter o login com a marca Conexão

1. **Criar o app no Azure** (portal.azure.com → *App registrations* → *New registration*)
   - Nome: `Conexão Launcher`
   - Tipo de conta: *Personal Microsoft accounts only*
   - Redirect URI: tipo *Public client/native*, valor `http://localhost:47837/callback`
   - Anote o **Application (client) ID**.
2. **Pedir permissão à Mojang para usar a API do Minecraft**
   - Formulário: https://aka.ms/mce-reviewappid (informar o client ID e explicar o Conexão).
   - Sem essa aprovação o login falha com "app não permitido". A análise pode levar semanas —
     **vale pedir já**.
3. **Quando aprovado**, me envie o client ID: troco o fluxo do launcher para o
   OAuth padrão (Microsoft → Xbox Live → XSTS → Minecraft) com o app do Conexão,
   mantendo o atual como reserva.

Nada disso bloqueia os testes nem o lançamento em beta.

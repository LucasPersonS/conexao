# Conexão — o launcher da comunidade de Minecraft Roleplay

O Conexão é um launcher de Minecraft no estilo do **FiveM**: o jogador abre, escolhe um servidor de RP e clica em **Jogar**. O launcher cuida do resto.

> Status: em desenvolvimento, com testes locais funcionando. Ainda não está publicado.

---

## Para o jogador

- **Entra com a conta Microsoft** (Minecraft original).
- **Vê os servidores parceiros** com banner, jogadores online, estilo (medieval, cidade, polícia…) e destaques.
- **Clica em Jogar.** O Conexão instala sozinho o Java, o loader (NeoForge, Forge ou Fabric) e os mods, e abre o Minecraft **já conectado ao servidor**.
- **Pede whitelist pelo próprio launcher** e acompanha o status: *aguardando*, *aprovado*…
- **Nas próximas vezes**, só baixa o que mudou no modpack.

## Para o dono do servidor (parceiro)

Pouco trabalho, de propósito:

1. **No painel web**, digita o endereço do servidor. O painel detecta versão, MOTD e vagas.
2. **Arrasta a pasta do modpack** (ou um `.mrpack`). Minecraft, loader e versões são detectados sozinhos.
3. **Revisa e publica.** Os jogadores recebem a atualização no próximo *Jogar*.

**Opcional: o Conexão Companion**, um mod que fica só no servidor:

- **Vínculo:** o dono coloca o mod, reinicia e clica no link que aparece no console.
- **Whitelist:** a whitelist aprovada no painel passa a valer na hora.
- **Modpack:** o painel avisa quando os mods do servidor mudaram e atualiza o modpack com um clique.

**No painel**, o dono também cuida de:

- **Whitelist:** pedidos, aprovar, recusar e banir.
- **Equipe:** admins e moderadores.
- **Modpack:** versões estável e beta, com rollback em um clique.
- **Histórico:** quem fez o quê.

## Para a equipe Conexão

- **Aprovar servidores:** um servidor só aparece no launcher depois que a equipe aprova.
- **Destaques:** ligar o **destaque** (vitrine da home). O cadastro é gratuito; o destaque é pago.
- **Espaço por servidor:** cada servidor tem uma cota. Mods do Modrinth são baixados direto de lá, então o Conexão só guarda configs e arquivos próprios, sem sobrecarregar a VPS.

## Segurança

- **Modpack assinado:** cada modpack publicado é assinado digitalmente, e o launcher recusa arquivos alterados.
- **Tipos de arquivo:** executáveis e arquivos fora do padrão são bloqueados.
- **Login:** só contas originais (login Microsoft verificado).

## Suporte

- **Minecraft:** 1.18.2 ou mais novo (inclui a numeração nova, 26.x).
- **Loaders:** NeoForge, Forge e Fabric.
- **Companion:** Fabric, Forge e NeoForge, do 1.18.2 ao 26.x (o painel oferece o JAR certo para cada servidor).

## Próximos passos

1. Testes com servidores e modpacks reais.
2. Publicação na VPS (domínio, HTTPS, instalador do launcher).
3. Pagamento automático do destaque.

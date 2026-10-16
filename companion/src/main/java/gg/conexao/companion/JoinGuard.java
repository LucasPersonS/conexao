package gg.conexao.companion;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.net.UnknownHostException;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import org.jetbrains.annotations.Nullable;

/**
 * Passe de entrada para contas não originais.
 *
 * <p>Em {@code online-mode=false} o servidor não confere quem é o jogador: qualquer um
 * entraria com o nick de outra pessoa. Quando o dono liga "Aceitar contas não originais"
 * no painel, o launcher pede um passe ao Conexão antes de abrir o jogo e a API o repassa
 * para cá na hora. Só entra o nick que tem passe vindo daquele IP.
 *
 * <p>A conferência acontece antes do jogador entrar no mundo ({@code PlayerList#canPlayerLogin},
 * via mixin), junto com ban e whitelist: um impostor nem chega a derrubar o jogador
 * verdadeiro. É local, sem chamada de rede. Quem estava jogando pode reconectar pelo próprio
 * jogo por alguns minutos depois de sair.
 */
public final class JoinGuard {
    /** Única instância (o mixin do {@code PlayerList} consulta esta). */
    public static final JoinGuard INSTANCE = new JoinGuard();

    /** Depois de sair do servidor, por quanto tempo dá para voltar sem o launcher. */
    private static final long RECONNECT_MILLIS = 10 * 60_000L;

    private record Pass(InetAddress ip, long expiresAt) {}

    private final Map<String, Pass> passes = new ConcurrentHashMap<>();
    /** Nicks que entraram com passe e ainda estão no servidor. */
    private final Set<String> online = ConcurrentHashMap.newKeySet();
    private volatile boolean enforce;

    private JoinGuard() {}

    /** {@code join_tickets}: liga/desliga e substitui os passes conhecidos. */
    void reset(JsonObject message) {
        boolean wasEnforcing = enforce;
        enforce = message.get("enforce").getAsBoolean();
        long now = System.currentTimeMillis();
        // Quem está jogando continua liberado; o resto vem da lista da API.
        passes.keySet().retainAll(online);
        JsonArray tickets = message.getAsJsonArray("tickets");
        if (tickets != null) {
            for (JsonElement ticket : tickets) {
                add(ticket.getAsJsonObject());
            }
        }
        passes.values().removeIf(pass -> pass.expiresAt() <= now);
        if (enforce != wasEnforcing) {
            ConexaoCompanion.LOG.info(enforce
                    ? "Contas não originais ativadas: só entra quem abrir o jogo pelo launcher do Conexão."
                    : "Contas não originais desativadas no painel.");
        }
    }

    /** {@code join_ticket}: passe novo (o jogador acabou de clicar em Jogar). */
    void add(JsonObject ticket) {
        String name = ticket.get("name").getAsString();
        long expiresIn = Math.max(0, ticket.get("expires_in").getAsLong());
        try {
            InetAddress ip = InetAddress.getByName(ticket.get("ip").getAsString());
            long expiresAt = System.currentTimeMillis() + expiresIn * 1000;
            passes.merge(key(name), new Pass(ip, expiresAt),
                    (old, fresh) -> old.expiresAt() == Long.MAX_VALUE ? old : fresh);
        } catch (UnknownHostException e) {
            ConexaoCompanion.LOG.warn("Passe de entrada com IP inválido para {}", name);
        }
    }

    boolean enforcing() {
        return enforce;
    }

    /**
     * Motivo para recusar a entrada, ou {@code null} para liberar. Chamado pelo mixin na
     * thread do servidor, só quando ban e whitelist já liberaram.
     */
    public @Nullable Component refusal(MinecraftServer server, SocketAddress address, String name) {
        if (!enforce || server.usesAuthentication()) {
            return null; // online-mode=true: a Mojang já confere quem é quem
        }
        Pass pass = passes.get(key(name));
        InetAddress ip = address instanceof InetSocketAddress inet ? inet.getAddress() : null;
        if (pass != null && pass.expiresAt() > System.currentTimeMillis() && pass.ip().equals(ip)) {
            passes.put(key(name), new Pass(pass.ip(), Long.MAX_VALUE)); // vale enquanto estiver jogando
            online.add(key(name));
            return null;
        }
        ConexaoCompanion.LOG.warn("{} ({}) tentou entrar sem passe do Conexão; recusado.", name,
                ip == null ? address : ip.getHostAddress());
        return Component.literal("Entre pelo launcher do Conexão.\n\n"
                + "Este servidor só libera quem clica em Jogar no Conexão (conexaosmps.com.br).");
    }

    /** Saiu: ainda pode voltar pelo próprio jogo por alguns minutos. */
    void onLogout(String name) {
        if (!online.remove(key(name))) {
            return;
        }
        long until = System.currentTimeMillis() + RECONNECT_MILLIS;
        passes.computeIfPresent(key(name), (k, pass) -> new Pass(pass.ip(), Math.min(pass.expiresAt(), until)));
    }

    private static String key(String name) {
        return name.toLowerCase(Locale.ROOT);
    }
}

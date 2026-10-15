package gg.conexao.companion;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.UUID;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.dedicated.DedicatedServer;
import net.minecraft.server.players.PlayerList;
import net.minecraft.server.players.UserWhiteList;
import net.minecraft.server.players.UserWhiteListEntry;

/**
 * Aplica a whitelist aprovada no painel direto no servidor (sem RCON, sem reiniciar).
 * Entradas colocadas à mão pelo dono continuam lá: o Companion só remove quem ele mesmo adicionou.
 */
final class WhitelistSync {
    private WhitelistSync() {}

    /** Deve rodar na thread do servidor. Retorna quantos jogadores o Conexão liberou. */
    static int apply(MinecraftServer server, CompanionConfig config, JsonArray players, Boolean required) {
        PlayerList list = server.getPlayerList();
        UserWhiteList whitelist = list.getWhiteList();
        Set<String> desired = new LinkedHashSet<>();
        for (JsonElement element : players) {
            JsonObject player = element.getAsJsonObject();
            UUID uuid = UUID.fromString(player.get("uuid").getAsString());
            String name = player.get("name").getAsString();
            GameProfile profile = new GameProfile(uuid, name);
            desired.add(uuid.toString());
            if (!whitelist.isWhiteListed(profile)) {
                whitelist.add(new UserWhiteListEntry(profile));
            }
        }
        for (String uuid : config.managedWhitelist) {
            if (!desired.contains(uuid)) {
                whitelist.remove(new GameProfile(UUID.fromString(uuid), ""));
            }
        }
        config.managedWhitelist = desired;
        config.save();

        if (required != null) {
            if (list.isUsingWhitelist() != required) {
                list.setUsingWhiteList(required);
                if (server instanceof DedicatedServer dedicated) {
                    dedicated.storeUsingWhiteList(required); // grava no server.properties
                }
                ConexaoCompanion.LOG.info("Whitelist do servidor {} pelo Conexão.", required ? "ligada" : "desligada");
            }
            if (required) {
                server.setEnforceWhitelist(true);
                server.kickUnlistedPlayers(server.createCommandSourceStack());
            }
        }
        return desired.size();
    }
}

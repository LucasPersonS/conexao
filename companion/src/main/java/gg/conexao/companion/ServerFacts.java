package gg.conexao.companion;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.stream.Stream;
import net.minecraft.SharedConstants;
import net.minecraft.server.MinecraftServer;
import net.neoforged.fml.ModList;
import net.neoforged.fml.loading.FMLPaths;

/** Lê do servidor em execução tudo o que o painel precisaria perguntar ao dono. */
final class ServerFacts {
    private ServerFacts() {}

    static JsonObject info(MinecraftServer server) {
        JsonObject info = new JsonObject();
        info.addProperty("type", "server_info");
        info.addProperty("minecraft", SharedConstants.getCurrentVersion().getName());
        info.addProperty("loader", "neoforge");
        ModList.get()
                .getModContainerById("neoforge")
                .ifPresent(c -> info.addProperty("loader_version", c.getModInfo().getVersion().toString()));
        info.addProperty("motd", server.getMotd() == null ? "" : server.getMotd());
        info.addProperty("max_players", server.getMaxPlayers());
        info.addProperty("port", Math.max(0, server.getPort()));
        info.addProperty("online_mode", server.usesAuthentication());
        info.addProperty("whitelist", server.getPlayerList().isUsingWhitelist());
        return info;
    }

    /** {@code mods/*.jar} → SHA-1 (sem o próprio Companion, que não vai para os jogadores). */
    static Map<String, Path> scanMods() {
        Path dir = FMLPaths.MODSDIR.get();
        Path self = ModList.get().getModFileById(ConexaoCompanion.MOD_ID) == null
                ? null
                : ModList.get().getModFileById(ConexaoCompanion.MOD_ID).getFile().getFilePath();
        Map<String, Path> result = new LinkedHashMap<>();
        if (!Files.isDirectory(dir)) {
            return result;
        }
        try (Stream<Path> files = Files.list(dir)) {
            files.filter(p -> Files.isRegularFile(p) && p.getFileName().toString().toLowerCase().endsWith(".jar"))
                    .filter(p -> self == null || !isSame(p, self))
                    .sorted()
                    .forEach(p -> {
                        try {
                            result.put(sha1(p), p);
                        } catch (IOException e) {
                            ConexaoCompanion.LOG.warn("Não foi possível ler {}: {}", p, e.toString());
                        }
                    });
        } catch (IOException e) {
            ConexaoCompanion.LOG.warn("Não foi possível listar {}: {}", dir, e.toString());
        }
        return result;
    }

    static JsonObject snapshot(Map<String, Path> mods) {
        JsonArray files = new JsonArray();
        mods.forEach((sha1, path) -> {
            JsonObject file = new JsonObject();
            file.addProperty("path", "mods/" + path.getFileName());
            file.addProperty("sha1", sha1);
            try {
                file.addProperty("size", Files.size(path));
            } catch (IOException e) {
                file.addProperty("size", 0);
            }
            files.add(file);
        });
        JsonObject message = new JsonObject();
        message.addProperty("type", "mods_snapshot");
        message.add("files", files);
        return message;
    }

    private static boolean isSame(Path a, Path b) {
        try {
            return Files.isSameFile(a, b);
        } catch (IOException e) {
            return false;
        }
    }

    static String sha1(Path path) throws IOException {
        try (InputStream in = Files.newInputStream(path)) {
            MessageDigest digest = MessageDigest.getInstance("SHA-1");
            byte[] buffer = new byte[64 * 1024];
            int read;
            while ((read = in.read(buffer)) != -1) {
                digest.update(buffer, 0, read);
            }
            return HexFormat.of().formatHex(digest.digest());
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }
}

package gg.conexao.companion;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.io.IOException;
import java.io.InputStream;
import java.io.Reader;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.SecureRandom;
import java.util.HexFormat;
import java.util.LinkedHashSet;
import java.util.Properties;
import java.util.Set;
import net.neoforged.fml.loading.FMLPaths;

/** {@code config/conexao-companion.json}: criado sozinho, ninguém precisa editar. */
public final class CompanionConfig {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();

    /** API do Conexão. Vazio = a do build (produção). */
    public String apiUrl = "";
    /** Segredo usado só no vínculo inicial. */
    public String secret = "";
    /** Token do agente, recebido depois que o dono confirma o vínculo no painel. */
    public String token = "";
    /** Servidor vinculado (só para exibir no console). */
    public String server = "";
    /** Nomes que o Conexão colocou na whitelist (as entradas manuais do dono não são tocadas). */
    public Set<String> managedWhitelist = new LinkedHashSet<>();

    private transient Path file;

    public static CompanionConfig load() {
        Path file = FMLPaths.CONFIGDIR.get().resolve("conexao-companion.json");
        // Servidores vinculados quando o projeto se chamava CENTRAL: aproveita o token antigo.
        Path legacy = FMLPaths.CONFIGDIR.get().resolve("central-companion.json");
        Path source = !Files.exists(file) && Files.exists(legacy) ? legacy : file;
        CompanionConfig config = null;
        if (Files.exists(source)) {
            try (Reader reader = Files.newBufferedReader(source, StandardCharsets.UTF_8)) {
                config = GSON.fromJson(reader, CompanionConfig.class);
            } catch (IOException | RuntimeException e) {
                ConexaoCompanion.LOG.warn("Não foi possível ler {}: {}", source, e.toString());
            }
        }
        if (config == null) {
            config = new CompanionConfig();
        }
        if (config.managedWhitelist == null) {
            config.managedWhitelist = new LinkedHashSet<>();
        }
        config.file = file;
        if (config.secret == null || config.secret.length() < 32) {
            byte[] raw = new byte[32];
            new SecureRandom().nextBytes(raw);
            config.secret = HexFormat.of().formatHex(raw);
        }
        config.save();
        return config;
    }

    public synchronized void save() {
        try {
            Files.createDirectories(file.getParent());
            Path tmp = file.resolveSibling(file.getFileName() + ".tmp");
            try (Writer writer = Files.newBufferedWriter(tmp, StandardCharsets.UTF_8)) {
                GSON.toJson(this, writer);
            }
            Files.move(tmp, file, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        } catch (IOException e) {
            ConexaoCompanion.LOG.warn("Não foi possível salvar {}: {}", file, e.toString());
        }
    }

    /** API efetiva: variável de ambiente, config ou a embutida no build. */
    public String api() {
        String env = System.getenv("CONEXAO_API_URL");
        String value = env != null && !env.isBlank() ? env : apiUrl;
        if (value == null || value.isBlank()) {
            value = buildProperty("api_url", "https://api.conexao.gg");
        }
        return value.replaceAll("/+$", "");
    }

    public static String buildProperty(String key, String fallback) {
        try (InputStream in = CompanionConfig.class.getResourceAsStream("/conexao-companion.properties")) {
            if (in == null) {
                return fallback;
            }
            Properties props = new Properties();
            props.load(in);
            String value = props.getProperty(key);
            return value == null || value.isBlank() || value.contains("${") ? fallback : value;
        } catch (IOException e) {
            return fallback;
        }
    }
}

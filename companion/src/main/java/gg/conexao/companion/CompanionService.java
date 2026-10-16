package gg.conexao.companion;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.net.http.WebSocket;
import java.net.http.WebSocketHandshakeException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.time.Duration;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import net.minecraft.server.MinecraftServer;

/** Laço do Companion: vincular, conectar, ouvir a API, reconectar se cair. */
final class CompanionService {
    private static final List<String> ALLOWED_COMMANDS = List.of("whitelist ", "kick ", "say ", "list");
    private static final long MAX_UPLOAD = 200L * 1024 * 1024;

    private final MinecraftServer server;
    private final CompanionConfig config;
    private final HttpClient http;
    private final ExecutorService worker;
    /** Envios e uploads: a thread `worker` fica presa no laço da conexão. */
    private final ExecutorService tasks;
    private final String version = CompanionConfig.buildProperty("version", "dev");
    private volatile boolean running;
    private volatile WebSocket socket;
    private volatile Map<String, Path> mods = Map.of();
    private final JoinGuard guard;

    CompanionService(MinecraftServer server, CompanionConfig config, JoinGuard guard) {
        this.server = server;
        this.config = config;
        this.guard = guard;
        this.http = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(15)).build();
        this.worker = Executors.newSingleThreadExecutor(r -> {
            Thread thread = new Thread(r, "Conexão Companion");
            thread.setDaemon(true);
            return thread;
        });
        this.tasks = Executors.newFixedThreadPool(2, r -> {
            Thread thread = new Thread(r, "Conexão Companion tarefas");
            thread.setDaemon(true);
            return thread;
        });
    }

    void start() {
        running = true;
        worker.execute(this::loop);
    }

    void stop() {
        running = false;
        WebSocket ws = socket;
        if (ws != null) {
            ws.sendClose(WebSocket.NORMAL_CLOSURE, "servidor desligando");
        }
        worker.shutdownNow();
        tasks.shutdownNow();
    }

    private void loop() {
        long backoff = 5;
        mods = ServerFacts.scanMods();
        ConexaoCompanion.LOG.info("Conexão Companion {} — {} mod(s) encontrados, API {}", version, mods.size(), config.api());
        while (running) {
            try {
                if (config.token == null || config.token.isBlank()) {
                    link();
                } else {
                    connect();
                }
                backoff = 5;
            } catch (InterruptedException e) {
                return;
            } catch (Exception e) {
                ConexaoCompanion.LOG.warn("Conexão indisponível ({}). Nova tentativa em {} s.", e.getMessage(), backoff);
            }
            if (!running) {
                return;
            }
            try {
                Thread.sleep(backoff * 1000);
            } catch (InterruptedException e) {
                return;
            }
            backoff = Math.min(backoff * 2, 300);
        }
    }

    // ---------------------------------------------------------------- vínculo

    private void link() throws IOException, InterruptedException {
        JsonObject info = ServerFacts.info(server);
        info.remove("type");
        JsonObject hello = new JsonObject();
        hello.addProperty("secret", config.secret);
        hello.add("info", info);
        JsonObject response = post("/v1/companion/hello", hello);
        String url = response.get("link_url").getAsString();
        String code = response.get("code").getAsString();
        long lastBanner = 0;
        while (running) {
            if (System.currentTimeMillis() - lastBanner > 5 * 60_000) {
                banner(url, code);
                lastBanner = System.currentTimeMillis();
            }
            Thread.sleep(5_000);
            JsonObject secret = new JsonObject();
            secret.addProperty("secret", config.secret);
            JsonObject poll;
            try {
                poll = post("/v1/companion/poll", secret);
            } catch (HttpError e) {
                if (e.status == 404) {
                    return; // código expirou: o laço pede outro
                }
                throw e;
            }
            if (!"linked".equals(poll.get("status").getAsString())) {
                continue;
            }
            JsonElement token = poll.get("token");
            if (token == null || token.isJsonNull()) {
                // O token já foi entregue (config apagada?): gera outro segredo e recomeça.
                ConexaoCompanion.LOG.warn("Este código já foi usado. Gerando um novo vínculo…");
                config.secret = newSecret();
                config.save();
                return;
            }
            config.token = token.getAsString();
            config.server = poll.get("slug").getAsString();
            config.save();
            ConexaoCompanion.LOG.info("Servidor vinculado ao Conexão como \"{}\". Pronto!", config.server);
            return;
        }
    }

    private static void banner(String url, String code) {
        String line = "=".repeat(64);
        ConexaoCompanion.LOG.info(line);
        ConexaoCompanion.LOG.info(" Conexão: vincule este servidor ao painel (leva 1 minuto)");
        ConexaoCompanion.LOG.info(" Abra: {}", url);
        ConexaoCompanion.LOG.info(" ou digite o código {} em Painel → Vincular servidor", code);
        ConexaoCompanion.LOG.info(line);
    }

    private static String newSecret() {
        byte[] raw = new byte[32];
        new SecureRandom().nextBytes(raw);
        return HexFormat.of().formatHex(raw);
    }

    // -------------------------------------------------------------- websocket

    private void connect() throws InterruptedException {
        String ws = config.api().replaceFirst("^http", "ws") + "/v1/agent/connect";
        CompletableFuture<Void> closed = new CompletableFuture<>();
        try {
            socket = http.newWebSocketBuilder()
                    .header("Authorization", "Bearer " + config.token)
                    .connectTimeout(Duration.ofSeconds(15))
                    .buildAsync(URI.create(ws), new Listener(closed))
                    .join();
        } catch (CompletionException e) {
            if (e.getCause() instanceof WebSocketHandshakeException handshake
                    && handshake.getResponse().statusCode() == 401) {
                ConexaoCompanion.LOG.warn("O token do Conexão foi revogado. Será preciso vincular de novo.");
                config.token = "";
                config.save();
                return;
            }
            throw new IllegalStateException(e.getCause() == null ? e.getMessage() : e.getCause().getMessage());
        }
        ConexaoCompanion.LOG.info("Conectado ao Conexão ({}).", config.server.isBlank() ? "servidor vinculado" : config.server);
        JsonObject hello = new JsonObject();
        hello.addProperty("type", "hello");
        hello.addProperty("agent_version", "companion-neoforge-" + version);
        send(hello);
        send(ServerFacts.info(server));
        send(ServerFacts.snapshot(mods));
        try {
            closed.join();
        } finally {
            socket = null;
        }
        if (running) {
            ConexaoCompanion.LOG.info("Conexão com o Conexão caiu; reconectando…");
        }
    }

    private synchronized void send(JsonObject message) {
        WebSocket ws = socket;
        if (ws != null) {
            ws.sendText(message.toString(), true).join();
        }
    }

    private final class Listener implements WebSocket.Listener {
        private final CompletableFuture<Void> closed;
        private final StringBuilder buffer = new StringBuilder();

        Listener(CompletableFuture<Void> closed) {
            this.closed = closed;
        }

        @Override
        public CompletionStage<?> onText(WebSocket ws, CharSequence data, boolean last) {
            buffer.append(data);
            if (last) {
                String text = buffer.toString();
                buffer.setLength(0);
                try {
                    handle(JsonParser.parseString(text).getAsJsonObject());
                } catch (RuntimeException e) {
                    ConexaoCompanion.LOG.warn("Mensagem inválida do Conexão: {}", e.toString());
                }
            }
            ws.request(1);
            return null;
        }

        @Override
        public CompletionStage<?> onClose(WebSocket ws, int status, String reason) {
            closed.complete(null);
            return null;
        }

        @Override
        public void onError(WebSocket ws, Throwable error) {
            closed.complete(null);
        }
    }

    private void handle(JsonObject message) {
        String type = message.get("type").getAsString();
        switch (type) {
            case "ping" -> reply("pong");
            case "whitelist_sync" -> {
                JsonArray players = message.getAsJsonArray("players");
                Boolean required = message.has("required") && !message.get("required").isJsonNull()
                        ? message.get("required").getAsBoolean()
                        : null;
                server.execute(() -> {
                    try {
                        int count = WhitelistSync.apply(server, config, players, required);
                        JsonObject done = new JsonObject();
                        done.addProperty("type", "whitelist_applied");
                        done.addProperty("count", count);
                        CompletableFuture.runAsync(() -> send(done), tasks);
                        ConexaoCompanion.LOG.info("Whitelist do Conexão aplicada: {} jogador(es).", count);
                    } catch (RuntimeException e) {
                        error("falha ao aplicar a whitelist: " + e);
                    }
                });
            }
            case "join_tickets" -> {
                guard.reset(message);
                if (guard.enforcing() && server.enforceSecureProfile()) {
                    ConexaoCompanion.LOG.warn("enforce-secure-profile=true: contas não originais serão expulsas. "
                            + "Defina enforce-secure-profile=false no server.properties.");
                }
            }
            case "join_ticket" -> guard.add(message);
            case "upload_files" -> CompletableFuture.runAsync(() -> upload(message.getAsJsonArray("sha1")), tasks);
            case "command" -> command(message.get("id").getAsString(), message.get("command").getAsString());
            default -> ConexaoCompanion.LOG.debug("Mensagem ignorada: {}", type);
        }
    }

    private void reply(String type) {
        JsonObject message = new JsonObject();
        message.addProperty("type", type);
        CompletableFuture.runAsync(() -> send(message), tasks);
    }

    private void error(String text) {
        ConexaoCompanion.LOG.warn(text);
        JsonObject message = new JsonObject();
        message.addProperty("type", "error");
        message.addProperty("message", text);
        CompletableFuture.runAsync(() -> send(message), tasks);
    }

    private void command(String id, String command) {
        String trimmed = command.trim();
        boolean allowed = !trimmed.contains("\n") && !trimmed.contains("\r")
                && ALLOWED_COMMANDS.stream().anyMatch(p -> trimmed.equals(p.trim()) || trimmed.startsWith(p));
        JsonObject result = new JsonObject();
        result.addProperty("type", "command_result");
        result.addProperty("id", id);
        if (!allowed) {
            result.addProperty("ok", false);
            result.addProperty("output", "comando não permitido");
            CompletableFuture.runAsync(() -> send(result), tasks);
            return;
        }
        server.execute(() -> {
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), trimmed);
            result.addProperty("ok", true);
            result.addProperty("output", "");
            CompletableFuture.runAsync(() -> send(result), tasks);
        });
    }

    /** Envia ao Conexão os mods pedidos (os que não existem no Modrinth). */
    private void upload(JsonArray hashes) {
        JsonArray ok = new JsonArray();
        JsonArray failed = new JsonArray();
        for (JsonElement element : hashes) {
            String sha1 = element.getAsString();
            Path path = mods.get(sha1);
            try {
                if (path == null) {
                    throw new IOException("arquivo não está mais na pasta mods/");
                }
                if (Files.size(path) > MAX_UPLOAD) {
                    throw new IOException("arquivo grande demais");
                }
                if (!ServerFacts.sha1(path).equals(sha1)) {
                    throw new IOException("o arquivo mudou desde a última leitura");
                }
                HttpRequest request = HttpRequest.newBuilder(URI.create(config.api() + "/v1/files/" + sha1))
                        .header("Authorization", "Bearer " + config.token)
                        .header("Content-Type", "application/octet-stream")
                        .timeout(Duration.ofMinutes(10))
                        .PUT(HttpRequest.BodyPublishers.ofFile(path))
                        .build();
                HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
                if (response.statusCode() >= 300) {
                    throw new IOException("HTTP " + response.statusCode() + " " + response.body());
                }
                ok.add(sha1);
                ConexaoCompanion.LOG.info("Enviado ao Conexão: {}", path.getFileName());
            } catch (IOException | RuntimeException e) {
                failed.add(sha1);
                ConexaoCompanion.LOG.warn("Não foi possível enviar {}: {}", path == null ? sha1 : path.getFileName(), e.getMessage());
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return;
            }
        }
        JsonObject message = new JsonObject();
        message.addProperty("type", "files_uploaded");
        message.add("sha1", ok);
        message.add("failed", failed);
        send(message);
    }

    // ------------------------------------------------------------------- http

    private static final class HttpError extends IOException {
        final int status;

        HttpError(int status, String body) {
            super("HTTP " + status + ": " + body);
            this.status = status;
        }
    }

    private JsonObject post(String path, JsonObject body) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder(URI.create(config.api() + path))
                .header("Content-Type", "application/json")
                .header("User-Agent", "Conexao-Companion/" + version)
                .timeout(Duration.ofSeconds(20))
                .POST(HttpRequest.BodyPublishers.ofString(body.toString()))
                .build();
        HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() >= 300) {
            throw new HttpError(response.statusCode(), response.body());
        }
        return JsonParser.parseString(response.body()).getAsJsonObject();
    }
}

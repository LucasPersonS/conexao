package gg.conexao.companion;

import net.minecraft.server.MinecraftServer;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.loading.FMLEnvironment;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.entity.player.PlayerEvent;
import net.neoforged.neoforge.event.server.ServerStartedEvent;
import net.neoforged.neoforge.event.server.ServerStoppingEvent;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Conexão Companion: liga o servidor ao Conexão sem configuração.
 *
 * <p>Na primeira vez, mostra no console um link para o dono vincular o servidor
 * no painel. Depois disso mantém a whitelist sincronizada, informa versões e mods
 * instalados e envia ao Conexão os arquivos que não existem no Modrinth.
 *
 * <p>Só age em servidor dedicado; os jogadores não precisam do mod.
 */
@Mod(ConexaoCompanion.MOD_ID)
public final class ConexaoCompanion {
    public static final String MOD_ID = "conexao_companion";
    public static final Logger LOG = LoggerFactory.getLogger("Conexão");

    private CompanionService service;

    public ConexaoCompanion(IEventBus modBus) {
        if (FMLEnvironment.dist != Dist.DEDICATED_SERVER) {
            LOG.info("Conexão Companion só funciona em servidor dedicado; ignorado no cliente.");
            return;
        }
        NeoForge.EVENT_BUS.addListener(this::onStarted);
        NeoForge.EVENT_BUS.addListener(this::onStopping);
        NeoForge.EVENT_BUS.addListener(this::onLogout);
    }

    private void onLogout(PlayerEvent.PlayerLoggedOutEvent event) {
        if (event.getEntity() instanceof ServerPlayer player) {
            JoinGuard.INSTANCE.onLogout(player.getGameProfile().getName());
        }
    }

    private void onStarted(ServerStartedEvent event) {
        MinecraftServer server = event.getServer();
        service = new CompanionService(server, CompanionConfig.load(), JoinGuard.INSTANCE);
        service.start();
    }

    private void onStopping(ServerStoppingEvent event) {
        if (service != null) {
            service.stop();
            service = null;
        }
    }
}

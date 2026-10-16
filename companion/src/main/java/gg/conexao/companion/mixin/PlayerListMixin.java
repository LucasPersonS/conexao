package gg.conexao.companion.mixin;

import com.mojang.authlib.GameProfile;
import gg.conexao.companion.JoinGuard;
import java.net.SocketAddress;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.players.PlayerList;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Passe de entrada do Conexão junto com ban e whitelist: quem não tem passe é recusado
 * antes de entrar no mundo (e antes de derrubar quem já está jogando com o mesmo nick).
 */
@Mixin(PlayerList.class)
abstract class PlayerListMixin {
    @Shadow
    @Final
    private MinecraftServer server;

    @Inject(method = "canPlayerLogin", at = @At("RETURN"), cancellable = true)
    private void conexao$joinTicket(SocketAddress address, GameProfile profile, CallbackInfoReturnable<Component> cir) {
        if (cir.getReturnValue() == null) {
            Component refusal = JoinGuard.INSTANCE.refusal(this.server, address, profile.getName());
            if (refusal != null) {
                cir.setReturnValue(refusal);
            }
        }
    }
}

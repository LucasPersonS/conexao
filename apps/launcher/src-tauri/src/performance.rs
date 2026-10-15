//! Desempenho do jogo: memória automática e flags de JVM, aplicadas na
//! instância do servidor logo antes de abrir o Minecraft.

use std::path::Path;

use theseus::prelude::{EditInstance, InstanceLaunchOverridesPatch, MemorySettings};

use crate::error::CmdResult;
use crate::prefs::LauncherPrefs;

/// Flags de G1GC para cliente (as mesmas ideias do launcher oficial da Mojang):
/// coleta com pausas curtas, menos travadas ao carregar chunks e texturas.
pub const JVM_FLAGS: &[&str] = &[
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20",
    "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M",
];

/// Memória recomendada (MiB) para `mods` mods num computador com `system_mb` de RAM.
/// Deixa sempre espaço para o sistema, o Discord e o próprio launcher.
pub fn recommended_memory(mods: usize, system_mb: u32) -> u32 {
    let wanted: u32 = match mods {
        0 => 2048,
        1..=40 => 3072,
        41..=100 => 4096,
        101..=180 => 6144,
        _ => 8192,
    };
    let cap = (system_mb.saturating_sub(3072)).min(system_mb * 6 / 10);
    let floor = (system_mb / 2).min(2048);
    let value = wanted.min(cap).max(floor);
    value / 256 * 256
}

/// Conta os `.jar` em `mods/` (leitura de diretório, sem abrir os arquivos).
pub async fn count_mods(instance_dir: &Path) -> usize {
    let Ok(mut entries) = tokio::fs::read_dir(instance_dir.join("mods")).await else {
        return 0;
    };
    let mut count = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")) {
            count += 1;
        }
    }
    count
}

pub async fn system_memory_mb() -> u32 {
    theseus::jre::get_max_memory().await.map(|kib| u32::try_from(kib / 1024).unwrap_or(u32::MAX)).unwrap_or(8192)
}

/// Grava memória e flags na instância. Com as opções desligadas, a instância volta
/// a usar as configurações globais (o valor manual da tela de Configurações).
pub async fn apply(instance_id: &str, prefs: &LauncherPrefs) -> CmdResult<Option<u32>> {
    let dir = theseus::instance::get_full_path(instance_id).await?;
    let memory = if prefs.memory_auto {
        Some(recommended_memory(count_mods(&dir).await, system_memory_mb().await))
    } else {
        None
    };
    let args = prefs.jvm_optimized.then(|| JVM_FLAGS.iter().map(|f| (*f).to_string()).collect::<Vec<_>>());
    theseus::instance::edit(
        instance_id,
        EditInstance {
            launch_overrides: Some(InstanceLaunchOverridesPatch {
                memory: Some(memory.map(|maximum| MemorySettings { maximum })),
                extra_launch_args: Some(args),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await?;
    Ok(memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_follows_modpack_and_leaves_room() {
        assert_eq!(recommended_memory(0, 16384), 2048);
        assert_eq!(recommended_memory(30, 16384), 3072);
        assert_eq!(recommended_memory(150, 16384), 6144);
        assert_eq!(recommended_memory(400, 32768), 8192);
        // 8 GB: no máximo 4,75 GB para o jogo
        assert_eq!(recommended_memory(400, 8192), 4864);
        // 4 GB: 2 GB, o mínimo para modpacks
        assert_eq!(recommended_memory(150, 4096), 2048);
    }
}

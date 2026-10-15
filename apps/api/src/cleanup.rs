//! Limpeza diária do storage, para o disco da VPS não lotar com arquivos que ninguém usa.
//!
//! 1. Versões de servidores **recusados há mais de 7 dias** são apagadas (o modpack não
//!    tem jogadores; se o servidor voltar a ser aprovado, o dono publica de novo).
//! 2. Arquivos hospedados que **nenhuma versão publicada** usa (nem a última fotografia do
//!    agente) e que ninguém enviou ou consultou nas últimas 48 h são apagados: envios
//!    abandonados, versões que nunca foram publicadas, servidores apagados.
//! 3. Temporários de envios interrompidos com mais de 24 h somem de `tmp/`.
//!
//! Versões publicadas de servidores aprovados, em análise ou suspensos nunca perdem
//! arquivos: o rollback para uma versão antiga continua funcionando.
//!
//! Concorrência: consultar ou enviar um arquivo atualiza `last_used_at` (`UPDATE … RETURNING`),
//! e a limpeza apaga com `DELETE … WHERE last_used_at < …` arquivo por arquivo, removendo do
//! disco antes de confirmar a transação. Quem está publicando trava a linha antes da limpeza
//! (e ela desiste) ou espera a limpeza terminar (e vê que precisa enviar de novo).

use std::time::{Duration, SystemTime};

use crate::state::AppState;
use crate::storage;

const FIRST_RUN_AFTER: Duration = Duration::from_secs(15 * 60);
const RUN_EVERY: Duration = Duration::from_secs(24 * 3600);
/// Arquivo sem uso há menos que isto nunca é apagado.
const GRACE_HOURS: i32 = 48;
/// Dias depois da recusa até as versões do servidor serem apagadas.
const REJECTED_DAYS: i32 = 7;
/// Temporários mais velhos que isto são sobras de envios interrompidos.
const TMP_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
/// Arquivos por rodada (o resto fica para o dia seguinte).
const BATCH: i64 = 5000;

/// Condição de "arquivo sem uso" (`sf` = stored_files, `$1` = horas de margem).
const UNUSED: &str = "sf.last_used_at < now() - make_interval(hours => $1)
  AND sf.created_at < now() - make_interval(hours => $1)
  AND NOT EXISTS (SELECT 1 FROM pack_versions pv
                   WHERE pv.manifest -> 'files' @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1)))
  AND NOT EXISTS (SELECT 1 FROM server_snapshots ss
                   WHERE ss.mods @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1)))";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub versions: u64,
    pub files: u64,
    pub bytes: u64,
    pub temporaries: u64,
}

pub async fn run(state: AppState) {
    tokio::time::sleep(FIRST_RUN_AFTER).await;
    let mut interval = tokio::time::interval(RUN_EVERY);
    loop {
        interval.tick().await;
        match collect(&state).await {
            Ok(report) if report != Report::default() => tracing::info!(
                "limpeza do storage: {} versão(ões) de servidores recusados, {} arquivo(s) sem uso ({}), {} temporário(s)",
                report.versions,
                report.files,
                crate::policy::megabytes(report.bytes),
                report.temporaries
            ),
            Ok(_) => tracing::debug!("limpeza do storage: nada a apagar"),
            Err(error) => tracing::warn!("limpeza do storage falhou: {error:?}"),
        }
    }
}

pub async fn collect(state: &AppState) -> eyre::Result<Report> {
    let versions = sqlx::query(
        "DELETE FROM pack_versions pv USING servers s
          WHERE pv.server_id = s.id AND s.approval = 'rejected'
            AND s.approval_updated_at < now() - make_interval(days => $1)",
    )
    .bind(REJECTED_DAYS)
    .execute(&state.pool)
    .await?
    .rows_affected();
    let mut report = Report { versions, ..Report::default() };

    let candidates: Vec<String> =
        sqlx::query_scalar(&format!("SELECT sf.sha1 FROM stored_files sf WHERE {UNUSED} LIMIT $2"))
            .bind(GRACE_HOURS)
            .bind(BATCH)
            .fetch_all(&state.pool)
            .await?;
    let (files, bytes) = remove_if_unused(state, &candidates, GRACE_HOURS).await?;
    report.files = files;
    report.bytes = bytes;
    report.temporaries = remove_old_temporaries(&state.config.storage_dir.join("tmp")).await;
    Ok(report)
}

/// Apaga (banco e disco) os arquivos da lista que estão sem uso, com `grace_hours` de margem.
/// Retorna quantos arquivos e bytes saíram.
pub async fn remove_if_unused(state: &AppState, hashes: &[String], grace_hours: i32) -> eyre::Result<(u64, u64)> {
    let root = &state.config.storage_dir;
    let (mut files, mut bytes) = (0, 0);
    for sha1 in hashes {
        if !storage::is_sha1(sha1) {
            continue;
        }
        let mut tx = state.pool.begin().await?;
        // Confere dentro da transação: alguém pode ter usado o arquivo agora.
        let size: Option<i64> = sqlx::query_scalar(&format!(
            "DELETE FROM stored_files sf WHERE sf.sha1 = $2 AND {UNUSED} RETURNING sf.size"
        ))
        .bind(grace_hours)
        .bind(sha1)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(size) = size else { continue };
        match tokio::fs::remove_file(storage::file_path(root, sha1)).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!("não consegui apagar files/{sha1}: {error}");
                tx.rollback().await?;
                continue;
            }
        }
        tx.commit().await?;
        files += 1;
        bytes += u64::try_from(size).unwrap_or(0);
    }
    Ok((files, bytes))
}

async fn remove_old_temporaries(dir: &std::path::Path) -> u64 {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else { return 0 };
    let mut removed = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(meta) = entry.metadata().await else { continue };
        let old = meta
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .is_some_and(|age| age > TMP_MAX_AGE);
        if meta.is_file() && old && tokio::fs::remove_file(entry.path()).await.is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn removes_only_old_temporaries() {
        let dir = std::env::temp_dir().join(format!("conexao-tmp-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let old = dir.join("velho");
        let new = dir.join("novo");
        std::fs::write(&old, b"x").unwrap();
        std::fs::write(&new, b"x").unwrap();
        let two_days_ago = SystemTime::now() - Duration::from_secs(48 * 3600);
        std::fs::File::options().write(true).open(&old).unwrap().set_modified(two_days_ago).unwrap();
        assert_eq!(remove_old_temporaries(&dir).await, 1);
        assert!(!old.exists());
        assert!(new.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

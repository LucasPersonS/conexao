//! A chave pública do atualizador vem do mesmo lugar que o launcher usa
//! (apps/launcher/src-tauri/tauri.release.conf.json): o instalador só executa um
//! instalador assinado pelo Conexão.

fn main() {
    const RELEASE_CONF: &str = "../../launcher/src-tauri/tauri.release.conf.json";
    println!("cargo:rerun-if-changed={RELEASE_CONF}");
    println!("cargo:rerun-if-env-changed=CONEXAO_API_URL");
    let conf: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(RELEASE_CONF).expect("ler tauri.release.conf.json"))
            .expect("tauri.release.conf.json inválido");
    let pubkey = conf
        .pointer("/plugins/updater/pubkey")
        .and_then(|v| v.as_str())
        .filter(|k| !k.contains("COLE_AQUI"))
        .expect("chave pública do atualizador ausente em tauri.release.conf.json");
    println!("cargo:rustc-env=CONEXAO_UPDATER_PUBKEY={pubkey}");
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        match std::env::var("CONEXAO_API_URL") {
            Ok(url) if url.starts_with("https://") => {}
            _ => panic!("build de produção do instalador bloqueado: defina CONEXAO_API_URL com o endereço https da API"),
        }
    }
    tauri_build::build();
}

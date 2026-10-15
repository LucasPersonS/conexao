//! Um launcher de produção nunca sai sem a chave pública dos modpacks: sem ela,
//! ele instalaria arquivos sem conferir a assinatura do Conexão.

fn is_public_key(value: &str) -> bool {
    // Ed25519 = 32 bytes em base64 padrão (43 ou 44 caracteres).
    (43..=44).contains(&value.len())
        && value.trim_end_matches('=').bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

fn main() {
    println!("cargo:rerun-if-env-changed=CONEXAO_MANIFEST_PUBKEY");
    println!("cargo:rerun-if-env-changed=CONEXAO_API_URL");
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let mut problems = Vec::new();
        match std::env::var("CONEXAO_MANIFEST_PUBKEY") {
            Ok(key) if is_public_key(key.trim()) => {}
            Ok(key) if !key.trim().is_empty() => {
                problems.push("CONEXAO_MANIFEST_PUBKEY não parece uma chave pública Ed25519 em base64");
            }
            _ => problems.push("defina CONEXAO_MANIFEST_PUBKEY (saída de `conexao-api gerar-chave-assinatura`)"),
        }
        match std::env::var("CONEXAO_API_URL") {
            Ok(url) if url.starts_with("https://") => {}
            _ => problems.push("defina CONEXAO_API_URL com o endereço https da API"),
        }
        assert!(problems.is_empty(), "build de produção do launcher bloqueado:\n  - {}", problems.join("\n  - "));
    }
    tauri_build::build();
}

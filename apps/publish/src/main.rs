//! `conexao-publish`: publica uma nova versão do modpack de um servidor.
//!
//! 1. Analisa a pasta do modpack (mesmas regras do Publicador do Paravexia).
//! 2. Mostra o diff com a versão estável atual e os avisos.
//! 3. Envia só os arquivos que o Conexão ainda não tem.
//! 4. Registra a versão e aponta o canal escolhido para ela.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use conexao_pack::{ChangeKind, Group, PublishOptions, build_plan, scan};
use conexao_types::{FilePolicy, Loader, LoaderSpec, MinecraftVersion, PackManifest};
use clap::{Parser, ValueEnum};
use eyre::{WrapErr, bail, eyre};
use serde::Deserialize;

#[derive(Clone, Copy, ValueEnum)]
enum ChannelArg {
    Stable,
    Beta,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum LoaderArg {
    Fabric,
    Forge,
    Neoforge,
}

#[derive(Parser)]
#[command(name = "conexao-publish", about = "Publica uma versão do modpack no Conexão", disable_version_flag = true)]
struct Args {
    /// Identificador do servidor no Conexão (ex.: paravexia).
    #[arg(long)]
    server: String,
    /// Pasta raiz do modpack (a que contém mods/, config/...).
    #[arg(long)]
    dir: PathBuf,
    /// Nova versão (semver), ex.: 1.2.0.
    #[arg(long)]
    version: String,
    /// Versão do Minecraft (padrão: a da versão estável atual).
    #[arg(long)]
    minecraft: Option<String>,
    #[arg(long, value_enum)]
    loader: Option<LoaderArg>,
    #[arg(long)]
    loader_version: Option<String>,
    /// Pastas a publicar (padrão: as que existirem entre mods, config,
    /// defaultconfigs, resourcepacks, shaderpacks e kubejs). Pastas não
    /// analisadas mantêm os arquivos da versão anterior.
    #[arg(long, value_delimiter = ',')]
    groups: Vec<String>,
    /// Configs que devem sempre sobrescrever a cópia do jogador.
    #[arg(long = "managed")]
    managed: Vec<String>,
    /// Arquivos a deixar de fora.
    #[arg(long = "exclude")]
    exclude: Vec<String>,
    #[arg(long, value_enum, default_value = "stable")]
    channel: ChannelArg,
    /// Só mostra o plano, sem enviar nada.
    #[arg(long)]
    dry_run: bool,
    /// Não pede confirmação.
    #[arg(long, short)]
    yes: bool,
    #[arg(long, env = "CONEXAO_API_URL", default_value = "https://api.conexao.gg")]
    api: String,
    /// Token de sessão (no launcher: conta → Copiar token de publicação).
    #[arg(long, env = "CONEXAO_TOKEN", hide_env_values = true)]
    token: String,
}

#[derive(Deserialize)]
struct ServerInfo {
    name: String,
    minecraft: MinecraftVersion,
    loader: LoaderSpec,
}

#[derive(Deserialize)]
struct Missing {
    missing: Vec<String>,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
    #[serde(default)]
    details: serde_json::Value,
}

async fn check(response: reqwest::Response) -> eyre::Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    match response.json::<ApiError>().await {
        Ok(error) if error.details.is_null() => bail!("{} ({status})", error.message),
        Ok(error) => bail!("{} ({status}): {}", error.message, error.details),
        Err(_) => bail!("a API respondeu {status}"),
    }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let api = args.api.trim_end_matches('/').to_string();
    let http = reqwest::Client::builder()
        .user_agent(concat!("conexao-publish/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let auth = |request: reqwest::RequestBuilder| request.bearer_auth(&args.token);

    let server: ServerInfo = check(auth(http.get(format!("{api}/v1/servers/{}", args.server))).send().await?)
        .await
        .wrap_err("não foi possível ler o servidor; confira --server e o token")?
        .json()
        .await?;
    let previous_response = auth(http.get(format!("{api}/v1/servers/{}/manifest", args.server))).send().await?;
    let previous: Option<PackManifest> = if previous_response.status() == reqwest::StatusCode::NOT_FOUND {
        None
    } else {
        Some(check(previous_response).await?.json().await?)
    };

    let minecraft = match &args.minecraft {
        Some(v) => MinecraftVersion::parse_supported(v)?,
        None => previous.as_ref().map_or(server.minecraft, |p| p.minecraft),
    };
    let base_loader = previous.as_ref().map_or_else(|| server.loader.clone(), |p| p.loader.clone());
    let loader = LoaderSpec {
        kind: match args.loader {
            Some(LoaderArg::Fabric) => Loader::Fabric,
            Some(LoaderArg::Forge) => Loader::Forge,
            Some(LoaderArg::Neoforge) => Loader::NeoForge,
            None => base_loader.kind,
        },
        version: args.loader_version.clone().unwrap_or(base_loader.version),
    };

    let groups = if args.groups.is_empty() {
        Group::ALL.into_iter().filter(|g| args.dir.join(g.folder()).is_dir()).collect()
    } else {
        args.groups
            .iter()
            .map(|g| Group::from_folder(g.trim()).ok_or_else(|| eyre!("pasta desconhecida: {g}")))
            .collect::<eyre::Result<Vec<_>>>()?
    };
    if groups.is_empty() {
        bail!("nenhuma pasta de modpack (mods, config...) encontrada em {}", args.dir.display());
    }
    println!(
        "Analisando {} ({})…",
        args.dir.display(),
        groups.iter().map(|g| g.folder()).collect::<Vec<_>>().join(", ")
    );
    let scanned = scan(&args.dir, &groups, |_| {})?;
    let options = PublishOptions {
        server: args.server.clone(),
        name: server.name.clone(),
        version: args.version.clone(),
        minecraft,
        loader,
        excluded: args.exclude.iter().cloned().collect::<HashSet<_>>(),
        policies: args.managed.iter().map(|p| (p.clone(), FilePolicy::Managed)).collect::<HashMap<_, _>>(),
        sides: HashMap::new(),
        external_urls: HashMap::new(),
    };
    let plan = build_plan(previous.as_ref(), &scanned, &options)?;

    let count = |kind| plan.changes.iter().filter(|c| c.kind == kind).count();
    println!(
        "\n{} {} → versão {} ({} {} · Minecraft {})",
        server.name,
        previous.as_ref().map_or("(primeira publicação)".to_string(), |p| format!("de {}", p.version)),
        plan.manifest.version,
        plan.manifest.loader.kind.display_name(),
        plan.manifest.loader.version,
        plan.manifest.minecraft
    );
    println!(
        "  {} novos · {} alterados · {} removidos · {} sem mudança",
        count(ChangeKind::Added),
        count(ChangeKind::Changed),
        count(ChangeKind::Removed),
        count(ChangeKind::Unchanged)
    );
    for change in plan.changes.iter().filter(|c| c.kind != ChangeKind::Unchanged) {
        let sign = match change.kind {
            ChangeKind::Added => "+",
            ChangeKind::Changed => "~",
            ChangeKind::Removed => "-",
            ChangeKind::Unchanged => " ",
        };
        println!("  {sign} {}", change.path);
    }
    for warning in &plan.warnings {
        println!("  ! {warning}");
    }
    if args.dry_run {
        println!("\n--dry-run: nada foi enviado.");
        return Ok(());
    }
    if !args.yes {
        print!("\nPublicar? [s/N] ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_lowercase().as_str(), "s" | "sim" | "y" | "yes") {
            println!("Cancelado.");
            return Ok(());
        }
    }

    let hashes: Vec<&str> = plan.uploads.iter().map(|f| f.hashes.sha1.as_str()).collect();
    let missing: Missing = check(
        auth(http.post(format!("{api}/v1/files/missing")))
            .json(&serde_json::json!({ "sha1": hashes }))
            .send()
            .await?,
    )
    .await?
    .json()
    .await?;
    let to_send: Vec<_> = plan.uploads.iter().filter(|f| missing.missing.contains(&f.hashes.sha1)).collect();
    println!("\nEnviando {} arquivo(s) ({} já estavam no Conexão)", to_send.len(), plan.uploads.len() - to_send.len());
    for (index, file) in to_send.iter().enumerate() {
        println!("  [{}/{}] {}", index + 1, to_send.len(), file.relative);
        let bytes = tokio::fs::read(&file.absolute).await?;
        check(auth(http.put(format!("{api}/v1/files/{}", file.hashes.sha1))).body(bytes).send().await?).await?;
    }

    let channel = match args.channel {
        ChannelArg::Stable => Some("stable"),
        ChannelArg::Beta => Some("beta"),
        ChannelArg::None => None,
    };
    check(
        auth(http.post(format!("{api}/v1/servers/{}/versions", args.server)))
            .json(&serde_json::json!({ "manifest": plan.manifest, "channel": channel }))
            .send()
            .await?,
    )
    .await?;
    println!(
        "\nVersão {} publicada{}.",
        plan.manifest.version,
        channel.map_or(String::new(), |c| format!(" no canal {c}"))
    );
    Ok(())
}

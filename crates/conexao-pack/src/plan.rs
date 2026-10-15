use std::collections::{BTreeMap, HashMap, HashSet};

use conexao_types::manifest::MANIFEST_FORMAT;
use conexao_types::{FilePolicy, FileSide, LoaderSpec, MinecraftVersion, PackFile, PackManifest};
use chrono::Utc;

use crate::error::fail;
use crate::scan::{Group, Scan, ScannedFile};

/// O que o administrador escolheu na tela de publicação.
#[derive(Debug, Clone)]
pub struct PublishOptions {
    pub server: String,
    pub name: String,
    pub version: String,
    pub minecraft: MinecraftVersion,
    pub loader: LoaderSpec,
    /// Caminhos relativos que o admin desmarcou.
    pub excluded: HashSet<String>,
    /// Política por arquivo; sem entrada, configs são `initial` e o resto `managed`.
    pub policies: HashMap<String, FilePolicy>,
    pub sides: HashMap<String, FileSide>,
    /// URLs externas por SHA-1 (ex.: CDN do Modrinth), para não re-hospedar mods.
    pub external_urls: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Changed,
    Unchanged,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub manifest: PackManifest,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    /// Arquivos que precisam subir para o storage (sem URL externa), únicos por SHA-1.
    pub uploads: Vec<ScannedFile>,
}

fn valid_slug(slug: &str) -> bool {
    (2..=48).contains(&slug.len())
        && slug.bytes().next().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn top_folder(path: &str) -> &str {
    path.split('/').next().unwrap_or_default()
}

pub fn build_plan(previous: Option<&PackManifest>, scan: &Scan, options: &PublishOptions) -> crate::Result<Plan> {
    if !valid_slug(&options.server) {
        return fail("O identificador do servidor deve usar letras minúsculas, números e hífen (2 a 48 caracteres).");
    }
    if options.name.trim().is_empty() {
        return fail("Informe o nome do servidor.");
    }
    if options.version.starts_with('v') {
        return fail("Use uma versão como 1.0.1 ou 1.1.0-beta.1 (sem o prefixo v).");
    }
    let Ok(version) = semver::Version::parse(&options.version) else {
        return fail("Use uma versão como 1.0.1 ou 1.1.0-beta.1.");
    };
    if !options.minecraft.is_supported() {
        return fail("O Conexão suporta apenas Minecraft 1.18.2 ou mais novo.");
    }
    let loader_version = options.loader.version.trim();
    if loader_version.is_empty()
        || !loader_version.bytes().all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
    {
        return fail("Informe a versão do loader, por exemplo 21.1.256.");
    }

    let mut warnings = Vec::new();
    if let Some(previous) = previous {
        if previous.server != options.server {
            return fail("O manifesto anterior pertence a outro servidor.");
        }
        if let Ok(prior) = semver::Version::parse(&previous.version)
            && version <= prior
        {
            return fail(format!(
                "Escolha uma versão maior que a publicada ({}).",
                previous.version
            ));
        }
        if previous.minecraft != options.minecraft || previous.loader.kind != options.loader.kind {
            warnings.push(format!(
                "Troca de {} {} para {} {}: os jogadores farão uma reinstalação completa.",
                previous.loader.kind.display_name(),
                previous.minecraft,
                options.loader.kind.display_name(),
                options.minecraft
            ));
        }
    }

    // Arquivos de pastas que não foram analisadas continuam como estavam.
    let scanned_folders: HashSet<&str> = scan.groups.iter().map(|g| g.folder()).collect();
    let mut files: BTreeMap<String, PackFile> = previous
        .map(|p| {
            p.files
                .iter()
                .filter(|f| !scanned_folders.contains(top_folder(&f.path)))
                .map(|f| (f.path.to_lowercase(), f.clone()))
                .collect()
        })
        .unwrap_or_default();

    let mut uploads: BTreeMap<String, ScannedFile> = BTreeMap::new();
    let mut names: HashMap<String, String> = HashMap::new();
    for row in scan.files.iter().filter(|r| !options.excluded.contains(&r.relative)) {
        let policy = options.policies.get(&row.relative).copied().unwrap_or(if row.group.is_config() {
            FilePolicy::Initial
        } else {
            FilePolicy::Managed
        });
        let side = options.sides.get(&row.relative).copied().unwrap_or_default();
        let url = options.external_urls.get(&row.hashes.sha1).cloned();
        if let Some(url) = &url
            && !url.starts_with("https://")
        {
            return fail(format!("Use uma URL HTTPS pública para {}.", row.relative));
        }
        for warning in &row.warnings {
            warnings.push(format!("{}: {warning}", row.relative));
        }
        if let Some(info) = &row.mod_info {
            names.insert(row.relative.clone(), format!("{} {}", info.name, info.version));
            if !info.supports.contains(&options.loader.kind) {
                let hint = if options.loader.kind == conexao_types::Loader::NeoForge
                    && info.supports.contains(&conexao_types::Loader::Fabric)
                {
                    " Requer Sinytra Connector; inclua apenas se já testou o modpack."
                } else {
                    ""
                };
                warnings.push(format!(
                    "{}: feito para {}, mas o servidor usa {}.{hint}",
                    row.relative,
                    info.supports.iter().map(|l| l.display_name()).collect::<Vec<_>>().join("/"),
                    options.loader.kind.display_name()
                ));
            }
        }
        if url.is_none() {
            uploads.entry(row.hashes.sha1.clone()).or_insert_with(|| row.clone());
        }
        files.insert(
            row.relative.to_lowercase(),
            PackFile {
                path: row.relative.clone(),
                sha1: row.hashes.sha1.clone(),
                sha512: row.hashes.sha512.clone(),
                size: row.hashes.size,
                policy,
                side,
                url,
            },
        );
    }

    // Diff com a versão anterior.
    let old: HashMap<String, &PackFile> = previous
        .map(|p| p.files.iter().map(|f| (f.path.to_lowercase(), f)).collect())
        .unwrap_or_default();
    let mut changes = Vec::new();
    for (key, file) in &files {
        let kind = match old.get(key) {
            None => ChangeKind::Added,
            Some(prior) if prior.sha1 != file.sha1 || prior.policy != file.policy || prior.side != file.side => {
                ChangeKind::Changed
            }
            Some(_) => ChangeKind::Unchanged,
        };
        changes.push(Change {
            kind,
            path: file.path.clone(),
            name: names.get(&file.path).cloned().unwrap_or_else(|| file.path.clone()),
        });
    }
    for (key, file) in &old {
        if !files.contains_key(key) {
            changes.push(Change { kind: ChangeKind::Removed, path: file.path.clone(), name: file.path.clone() });
        }
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path));

    let included = |group: Group| {
        scan.files
            .iter()
            .any(|r| r.group == group && !options.excluded.contains(&r.relative))
    };
    if files.values().any(|f| f.policy == FilePolicy::Managed && Group::from_folder(top_folder(&f.path)).is_some_and(Group::is_config)) {
        warnings.push("Configs marcadas como gerenciadas substituirão alterações locais dos jogadores antes de abrir o jogo.".into());
    }
    if included(Group::ResourcePacks) {
        warnings.push("Resource packs serão instalados. A ativação deve ser feita no jogo ou exigida pelo servidor.".into());
    }
    if files.is_empty() {
        return fail("O modpack ficou vazio. Selecione ao menos um arquivo.");
    }

    let manifest = PackManifest {
        format: MANIFEST_FORMAT,
        server: options.server.clone(),
        name: options.name.trim().to_string(),
        version: options.version.clone(),
        minecraft: options.minecraft,
        loader: LoaderSpec { kind: options.loader.kind, version: loader_version.to_string() },
        files: files.into_values().collect(),
        created_at: Utc::now(),
    };
    Ok(Plan { manifest, changes, warnings, uploads: uploads.into_values().collect() })
}

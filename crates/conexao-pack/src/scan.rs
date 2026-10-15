use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{fail, io_err};
use crate::hash::{FileHashes, hash_file};
use crate::mods::{ModInfo, read_mod_info};

const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;

/// Pastas do modpack que o publicador sabe gerenciar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Mods,
    Config,
    DefaultConfigs,
    ResourcePacks,
    ShaderPacks,
    KubeJs,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Mods,
        Group::Config,
        Group::DefaultConfigs,
        Group::ResourcePacks,
        Group::ShaderPacks,
        Group::KubeJs,
    ];

    pub fn folder(self) -> &'static str {
        match self {
            Group::Mods => "mods",
            Group::Config => "config",
            Group::DefaultConfigs => "defaultconfigs",
            Group::ResourcePacks => "resourcepacks",
            Group::ShaderPacks => "shaderpacks",
            Group::KubeJs => "kubejs",
        }
    }

    pub fn from_folder(folder: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.folder() == folder)
    }

    /// Configs preservam as alterações do jogador por padrão (`initial`).
    pub fn is_config(self) -> bool {
        matches!(self, Group::Config | Group::DefaultConfigs)
    }
}

#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Caminho relativo à raiz do modpack, com `/`.
    pub relative: String,
    pub absolute: PathBuf,
    pub group: Group,
    pub hashes: FileHashes,
    pub mod_info: Option<ModInfo>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Scan {
    pub root: PathBuf,
    pub groups: Vec<Group>,
    pub files: Vec<ScannedFile>,
}

fn inside(root: &Path, target: &Path) -> bool {
    target.starts_with(root)
}

fn zip_has(path: &Path, entry: &str) -> bool {
    fs::File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .is_some_and(|mut z| z.by_name(entry).is_ok())
}

/// Analisa a pasta do modpack.
///
/// Regras herdadas do Paravexia: links simbólicos e arquivos fora da raiz são
/// recusados, nomes que colidem no Windows (maiúsculas/minúsculas) são
/// recusados, `mods/` só considera `.jar`, resource packs precisam ser ZIP.
pub fn scan(root: &Path, groups: &[Group], mut progress: impl FnMut(&str)) -> crate::Result<Scan> {
    let root = fs::canonicalize(root).map_err(io_err(root))?;
    if !root.is_dir() {
        return fail("Selecione a pasta raiz do modpack.");
    }
    if groups.is_empty() {
        return fail("Escolha pelo menos uma categoria de arquivos.");
    }
    let mut groups: Vec<Group> = groups.to_vec();
    groups.sort();
    groups.dedup();

    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for &group in &groups {
        let folder = root.join(group.folder());
        let Ok(meta) = fs::symlink_metadata(&folder) else {
            return fail(format!(
                "A categoria {} foi selecionada, mas essa pasta não existe. Desmarque-a para preservar os arquivos publicados.",
                group.folder()
            ));
        };
        if meta.file_type().is_symlink() {
            return fail(format!("A pasta {} é um link simbólico.", group.folder()));
        }
        walk(&root, &folder, group, &mut seen, &mut files, &mut progress)?;
    }

    // Mods duplicados (dois JARs declarando o mesmo modId).
    let mut by_id: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, file) in files.iter().enumerate() {
        if let Some(info) = &file.mod_info {
            let unique: HashSet<&String> = info.mod_ids.iter().collect();
            for id in unique {
                by_id.entry(id.clone()).or_default().push(index);
            }
        }
    }
    for (id, indexes) in by_id {
        if indexes.len() > 1 {
            for index in indexes {
                if let Some(file) = files.get_mut(index) {
                    file.warnings.push(format!(
                        "Mais de um JAR declara o mod {id}. Confira a duplicação antes de publicar."
                    ));
                }
            }
        }
    }

    Ok(Scan { root, groups, files })
}

fn walk(
    root: &Path,
    directory: &Path,
    group: Group,
    seen: &mut HashSet<String>,
    out: &mut Vec<ScannedFile>,
    progress: &mut impl FnMut(&str),
) -> crate::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .map_err(io_err(directory))?
        .collect::<Result<_, _>>()
        .map_err(io_err(directory))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let absolute = entry.path();
        let file_type = entry.file_type().map_err(io_err(&absolute))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_symlink() {
            return fail(format!("Links simbólicos não são aceitos: {name}"));
        }
        let real = fs::canonicalize(&absolute).map_err(io_err(&absolute))?;
        if !inside(root, &real) {
            return fail("Um arquivo aponta para fora da pasta selecionada.");
        }
        if file_type.is_dir() {
            walk(root, &absolute, group, seen, out, progress)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let relative = real
            .strip_prefix(root)
            .unwrap_or(&real)
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if relative.chars().any(|c| c.is_control() || c == ':' || c == '\\') {
            return fail(format!("Nome de arquivo inválido: {relative}"));
        }
        if !seen.insert(relative.to_lowercase()) {
            return fail(format!("Nomes que colidem no Windows: {relative}"));
        }
        let lower = relative.to_lowercase();
        if group == Group::Mods && !lower.ends_with(".jar") {
            continue;
        }
        if matches!(group, Group::ResourcePacks | Group::ShaderPacks) && !lower.ends_with(".zip") {
            return fail(format!("Compacte o pacote como ZIP antes de publicar: {relative}"));
        }

        progress(&format!("Lendo {relative}"));
        let hashes = hash_file(&absolute)?;
        if hashes.size > MAX_FILE {
            return fail(format!("Arquivo excede 2 GB: {relative}"));
        }
        let mut warnings = Vec::new();
        let mut mod_info = None;
        if group == Group::Mods {
            match read_mod_info(&absolute) {
                Ok(info) => mod_info = Some(info),
                Err(error) => warnings.push(format!(
                    "Não foi possível identificar os metadados: {error} Você pode publicar este JAR mesmo assim."
                )),
            }
        }
        if group == Group::ResourcePacks && !zip_has(&absolute, "pack.mcmeta") {
            warnings.push(
                "Resource pack sem pack.mcmeta na raiz. Você pode publicar o ZIP mesmo assim.".into(),
            );
        }
        out.push(ScannedFile { relative, absolute, group, hashes, mod_info, warnings });
    }
    Ok(())
}

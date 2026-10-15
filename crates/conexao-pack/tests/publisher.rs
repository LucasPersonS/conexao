use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use conexao_pack::{ChangeKind, Group, PublishOptions, build_plan, scan};
use conexao_types::{FilePolicy, Loader, LoaderSpec};

fn jar(path: &Path, entries: &[(&str, &str)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, body) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn modpack() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    jar(
        &root.join("mods/create.jar"),
        &[(
            "META-INF/neoforge.mods.toml",
            "[[mods]]\nmodId=\"create\"\nversion=\"6.0.4\"\ndisplayName=\"Create\"\n",
        )],
    );
    jar(
        &root.join("mods/sodium.jar"),
        &[("fabric.mod.json", r#"{"id":"sodium","version":"0.6.0","name":"Sodium"}"#)],
    );
    write(&root.join("mods/readme.txt"), "ignorado");
    write(&root.join("config/create-client.toml"), "a = 1");
    jar(&root.join("resourcepacks/rp.zip"), &[("pack.mcmeta", "{}")]);
    dir
}

fn options(version: &str) -> PublishOptions {
    PublishOptions {
        server: "paravexia".into(),
        name: "Paravexia".into(),
        version: version.into(),
        minecraft: "1.21.1".parse().unwrap(),
        loader: LoaderSpec { kind: Loader::NeoForge, version: "21.1.256".into() },
        excluded: HashSet::new(),
        policies: HashMap::new(),
        sides: HashMap::new(),
        external_urls: HashMap::new(),
    }
}

const GROUPS: [Group; 3] = [Group::Mods, Group::Config, Group::ResourcePacks];

#[test]
fn scans_mods_configs_and_resourcepacks() {
    let dir = modpack();
    let scanned = scan(dir.path(), &GROUPS, |_| {}).unwrap();
    let paths: Vec<_> = scanned.files.iter().map(|f| f.relative.as_str()).collect();
    assert_eq!(
        paths,
        ["mods/create.jar", "mods/sodium.jar", "config/create-client.toml", "resourcepacks/rp.zip"]
    );
    let create = &scanned.files[0];
    let info = create.mod_info.as_ref().unwrap();
    assert_eq!(info.mod_ids, ["create"]);
    assert_eq!(info.version, "6.0.4");
    assert_eq!(create.hashes.sha1.len(), 40);
    assert_eq!(create.hashes.sha512.len(), 128);
}

#[test]
fn first_publish_defaults_and_warnings() {
    let dir = modpack();
    let scanned = scan(dir.path(), &GROUPS, |_| {}).unwrap();
    let plan = build_plan(None, &scanned, &options("1.0.0")).unwrap();

    let config = plan.manifest.files.iter().find(|f| f.path.starts_with("config/")).unwrap();
    assert_eq!(config.policy, FilePolicy::Initial, "configs preservam alterações do jogador");
    let create = plan.manifest.files.iter().find(|f| f.path == "mods/create.jar").unwrap();
    assert_eq!(create.policy, FilePolicy::Managed);

    assert!(plan.changes.iter().all(|c| c.kind == ChangeKind::Added));
    assert!(plan.warnings.iter().any(|w| w.contains("Sinytra Connector")), "{:?}", plan.warnings);
    assert_eq!(plan.uploads.len(), 4);
}

#[test]
fn second_publish_diffs_and_requires_bigger_version() {
    let dir = modpack();
    let scanned = scan(dir.path(), &GROUPS, |_| {}).unwrap();
    let first = build_plan(None, &scanned, &options("1.0.0")).unwrap().manifest;

    let err = build_plan(Some(&first), &scanned, &options("1.0.0")).unwrap_err();
    assert!(err.to_string().contains("maior"));

    fs::remove_file(dir.path().join("mods/sodium.jar")).unwrap();
    write(&dir.path().join("config/create-client.toml"), "a = 2");
    let scanned = scan(dir.path(), &GROUPS, |_| {}).unwrap();
    let plan = build_plan(Some(&first), &scanned, &options("1.1.0")).unwrap();
    let kind = |p: &str| plan.changes.iter().find(|c| c.path == p).unwrap().kind;
    assert_eq!(kind("mods/sodium.jar"), ChangeKind::Removed);
    assert_eq!(kind("config/create-client.toml"), ChangeKind::Changed);
    assert_eq!(kind("mods/create.jar"), ChangeKind::Unchanged);
}

#[test]
fn unscanned_folders_are_preserved() {
    let dir = modpack();
    let all = scan(dir.path(), &GROUPS, |_| {}).unwrap();
    let first = build_plan(None, &all, &options("1.0.0")).unwrap().manifest;

    let only_mods = scan(dir.path(), &[Group::Mods], |_| {}).unwrap();
    let plan = build_plan(Some(&first), &only_mods, &options("1.0.1")).unwrap();
    assert!(plan.manifest.files.iter().any(|f| f.path == "config/create-client.toml"));
    assert!(plan.changes.iter().all(|c| c.kind != ChangeKind::Removed));
}

#[test]
fn rejects_old_minecraft_and_missing_folders() {
    let dir = modpack();
    let scanned = scan(dir.path(), &GROUPS, |_| {}).unwrap();
    let mut opts = options("1.0.0");
    opts.minecraft = "1.16.5".parse().unwrap();
    assert!(build_plan(None, &scanned, &opts).is_err());

    let err = scan(dir.path(), &[Group::KubeJs], |_| {}).unwrap_err();
    assert!(err.to_string().contains("kubejs"));
}

#[cfg(unix)]
#[test]
fn rejects_symlinks() {
    let dir = modpack();
    std::os::unix::fs::symlink("/etc/hostname", dir.path().join("config/link.txt")).unwrap();
    let err = scan(dir.path(), &GROUPS, |_| {}).unwrap_err();
    assert!(err.to_string().contains("Links simbólicos"));
}

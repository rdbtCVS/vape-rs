use quote::quote;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use syn::{Item, visit_mut::VisitMut};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn root() -> PathBuf {
    Path::new(ROOT)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn checked(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("command failed ({status}): {command:?}").into());
    }
    Ok(())
}

fn capture(command: &mut Command) -> Result<Vec<u8>> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!("{command:?}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(output.stdout)
}

fn write_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() != Some(bytes) {
        fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(tmp, path)?;
    }
    Ok(())
}

fn config(root: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        root.join("compatibility.json"),
    )?)?)
}

fn string<'a>(config: &'a Value, key: &str) -> Result<&'a str> {
    config[key]
        .as_str()
        .ok_or_else(|| format!("missing compatibility field {key}").into())
}

fn expand(path: &Path, base: &Path, sources: &mut BTreeMap<String, String>) -> Result<syn::File> {
    let bytes = fs::read(path)?;
    sources.insert(
        path.strip_prefix(base)?.to_string_lossy().into_owned(),
        hash(&bytes),
    );
    let mut file = syn::parse_file(std::str::from_utf8(&bytes)?)?;
    if !file.attrs.is_empty() || file.shebang.is_some() {
        return Err(format!("file attributes/shebang unsupported: {}", path.display()).into());
    }
    for item in &mut file.items {
        if let Item::Mod(module) = item {
            if module.attrs.iter().any(|a| a.path().is_ident("path")) {
                return Err("#[path] modules are unsupported".into());
            }
            if module.content.is_none() {
                let directory = if path
                    .file_name()
                    .is_some_and(|n| n == "lib.rs" || n == "mod.rs")
                {
                    path.parent().unwrap().to_path_buf()
                } else {
                    path.with_extension("")
                };
                let direct = directory.join(format!("{}.rs", module.ident));
                let nested = directory.join(module.ident.to_string()).join("mod.rs");
                if direct.exists() == nested.exists() {
                    return Err(format!("missing/ambiguous module {}", module.ident).into());
                }
                let child = expand(
                    if direct.exists() { &direct } else { &nested },
                    base,
                    sources,
                )?;
                module.content = Some((syn::token::Brace::default(), child.items));
                module.semi = None;
            }
        }
    }
    Ok(file)
}

fn copy_sources(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    let mut entries = fs::read_dir(from)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            return Err("symlinks in bundles are unsupported".into());
        }
        if ty.is_dir() {
            copy_sources(&entry.path(), &to.join(entry.file_name()))?;
        } else {
            if !matches!(
                entry.path().extension().and_then(|e| e.to_str()),
                Some("rs" | "toml")
            ) {
                return Err("bundle source must be Rust or TOML".into());
            }
            write_changed(&to.join(entry.file_name()), &fs::read(entry.path())?)?;
        }
    }
    Ok(())
}

fn pack(root: &Path) -> Result<PathBuf> {
    let config = config(root)?;
    let cargo: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    if cargo["dependencies"]["serde_json"].as_str() != Some("=1.0.151") {
        return Err(
            "runtime serde_json contract changed; update dependency audit and payload lock".into(),
        );
    }
    let mut sources = BTreeMap::new();
    let file = expand(&root.join("src/lib.rs"), root, &mut sources)?;
    let payload = prettyplease::unparse(&file);
    let bundle = root.join("target/bundle/mixins/vape-rs");
    copy_sources(&root.join("mixins/vape-rs"), &bundle)?;
    for path in tree_hashes(&bundle)?.keys() {
        if path != Path::new("runtime.rs") && !root.join("mixins/vape-rs").join(path).is_file() {
            return Err(format!(
                "stale generated bundle file {}; remove target/bundle and repack",
                path.display()
            )
            .into());
        }
    }

    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(bundle.join("horizon-mixin.toml"))?)?;
    if manifest["compatibility"]["horizon_revision"].as_str() != config["horizon_revision"].as_str()
    {
        return Err("native manifest compatibility differs from compatibility.json".into());
    }
    write_changed(&bundle.join("runtime.rs"), payload.as_bytes())?;
    let native = json!({"schema":1,"id":"vape-rs","version":manifest["mod"]["version"].as_str(),
        "game":config["minecraft"],"horizon":config["horizon_revision"],"mixins":config["horizon_mixins_revision"]});
    write_changed(
        &root.join("target/bundle/horizon-native.json"),
        &serde_json::to_vec_pretty(&native)?,
    )?;
    write_changed(
        &root.join("target/payload-map.json"),
        &serde_json::to_vec_pretty(&json!({
            "schema":1,"sources":sources,"payload_sha256":hash(payload.as_bytes()),"mapping":"source module paths and content hashes; generated lines follow prettyplease formatting"
        }))?,
    )?;
    Ok(bundle)
}

fn payload_test(root: &Path) -> Result<()> {
    let bundle = pack(root)?;
    let fixture = root.join("target/payload-test");
    fs::create_dir_all(&fixture)?;
    let manifest = format!(
        "[workspace]\n[package]\nname=\"vape-rs\"\nversion=\"0.1.0-dev\"\nedition=\"2024\"\n[lib]\npath={:?}\n[dependencies]\nserde_json=\"=1.0.151\"\n",
        bundle
            .join("runtime.rs")
            .to_str()
            .ok_or("non-UTF8 payload path")?
    );
    write_changed(&fixture.join("Cargo.toml"), manifest.as_bytes())?;
    write_changed(
        &fixture.join("Cargo.lock"),
        &fs::read(root.join("tools/payload.lock"))?,
    )?;
    checked(
        Command::new("cargo")
            .args(["test", "--locked", "--offline", "--manifest-path"])
            .arg(fixture.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(root.join("target/payload-cargo")),
    )
}

fn export(root: &Path, horizon: &Path) -> Result<PathBuf> {
    let config = config(root)?;
    let pin = string(&config, "horizon_revision")?;
    let destination = root.join("target/horizon/pristine");
    let archive = root.join("target/horizon/pinned.tar");
    fs::create_dir_all(archive.parent().unwrap())?;
    // Export only Git-owned committed content, never a user's working tree or assets.
    checked(
        Command::new("git")
            .arg("-C")
            .arg(horizon)
            .arg("archive")
            .arg(pin)
            .arg("--output")
            .arg(&archive),
    )?;
    let fresh = root.join("target/horizon/export-new");
    if fresh.exists() {
        fs::remove_dir_all(&fresh)?;
    }
    fs::create_dir(&fresh)?;
    checked(
        Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&fresh),
    )?;
    remove_links(&fresh)?;
    fs::write(fresh.join(".horizon-revision"), pin)?;
    audit_hashes(&fresh, &config)?;
    if destination.exists() {
        if tree_hashes(&destination)? != tree_hashes(&fresh)? {
            return Err("pristine export differs from committed pin; remove only target/horizon/pristine and restage".into());
        }
        fs::remove_dir_all(fresh)?;
    } else {
        fs::rename(fresh, &destination)?;
    }
    Ok(destination)
}

fn tree_hashes(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    fn walk(base: &Path, path: &Path, result: &mut BTreeMap<PathBuf, String>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_symlink() {
                return Err("unexpected symlink in generated tree".into());
            }
            if ty.is_dir() {
                walk(base, &entry.path(), result)?;
            } else {
                result.insert(
                    entry.path().strip_prefix(base)?.to_path_buf(),
                    hash(&fs::read(entry.path())?),
                );
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result)?;
    Ok(result)
}

fn remove_links(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            fs::remove_file(entry.path())?;
        } else if ty.is_dir() {
            remove_links(&entry.path())?;
        }
    }
    Ok(())
}

fn audit_hashes(pristine: &Path, config: &Value) -> Result<()> {
    for (path, expected) in config["source_hashes"]
        .as_object()
        .ok_or("missing source hashes")?
    {
        let actual = hash(&fs::read(pristine.join(path))?);
        if expected.as_str() != Some(actual.as_str()) {
            return Err(format!("pinned source drift: {path}").into());
        }
    }
    Ok(())
}

fn stage(root: &Path, horizon: &Path) -> Result<PathBuf> {
    let bundle = pack(root)?;
    let pristine = export(root, horizon)?;
    let host_manifest: toml::Value = toml::from_str(&fs::read_to_string(
        pristine.join("crates/horizon-client/Cargo.toml"),
    )?)?;
    let host_lock: toml::Value = toml::from_str(&fs::read_to_string(pristine.join("Cargo.lock"))?)?;
    if host_manifest["dependencies"].get("serde_json").is_none()
        || !host_lock["package"]
            .as_array()
            .ok_or("host lock missing packages")?
            .iter()
            .any(|p| {
                p["name"].as_str() == Some("serde_json") && p["version"].as_str() == Some("1.0.151")
            })
    {
        return Err("pinned host must already provide serde_json 1.0.151; no dependency injection is performed".into());
    }
    let config = config(root)?;
    let mixins = root.join("target/upstream/horizon-mixins");
    let revision = capture(
        Command::new("git")
            .arg("-C")
            .arg(&mixins)
            .args(["rev-parse", "HEAD"]),
    )?;
    if String::from_utf8(revision)?.trim() != string(&config, "horizon_mixins_revision")? {
        return Err("wrong mixin tool revision".into());
    }
    let dirty = capture(Command::new("git").arg("-C").arg(&mixins).args([
        "status",
        "--porcelain",
        "--untracked-files=no",
    ]))?;
    if !dirty.is_empty() {
        return Err("mixin tool sources are dirty".into());
    }
    // Build reviewed pinned engine sources; never trust a stale preexisting executable.
    checked(
        Command::new("cargo")
            .args([
                "+1.95.0",
                "build",
                "--locked",
                "--offline",
                "-p",
                "horizon-mixins-cli",
                "--manifest-path",
            ])
            .arg(mixins.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(root.join("target/mixin-tool")),
    )?;
    let tool = root.join("target/mixin-tool/debug/horizon-mixins");
    let staged = root.join("target/horizon/modded");
    for (action, out) in [
        ("plan", root.join("target/horizon/plan.json")),
        ("apply", staged.clone()),
    ] {
        checked(
            Command::new(&tool)
                .arg(action)
                .arg("--workspace")
                .arg(&pristine)
                .arg("--mods")
                .arg(bundle.parent().unwrap())
                .arg("--out")
                .arg(out),
        )?;
    }
    audit(root)?;
    Ok(staged)
}

// Strip exactly our generated hooks/module from the transformed AST and compare
// with the original. All other Rust items, including cfg branches, must match.
fn audit(root: &Path) -> Result<()> {
    let config = config(root)?;
    let pristine = root.join("target/horizon/pristine");
    let staged = root.join("target/horizon/modded");
    audit_hashes(&pristine, &config)?;
    let plan: Value = serde_json::from_slice(&fs::read(root.join("target/horizon/plan.json"))?)?;
    let injections = plan["injections"]
        .as_array()
        .ok_or("missing plan injections")?;
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(
        root.join("mixins/vape-rs/horizon-mixin.toml"),
    )?)?;
    let declarations = manifest["injection"]
        .as_array()
        .ok_or("missing declared hooks")?;
    if injections.len() != declarations.len() || plan["modules"].as_array().map(Vec::len) != Some(1)
    {
        return Err("unexpected mixin plan shape".into());
    }
    let mut expected = Vec::new();
    for declaration in declarations {
        let id = declaration["id"].as_str().ok_or("missing hook id")?;
        let injection = injections
            .iter()
            .find(|p| p["injection_id"] == id)
            .ok_or("declared hook missing from plan")?;
        let bindings = declaration
            .get("bindings")
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or_else(|| json!([]));
        if declaration["kind"].as_str() != Some("HEAD")
            || injection["kind"] != "HEAD"
            || injection["target"].as_str() != declaration["target"].as_str()
            || injection["bindings"] != bindings
            || injection["cancellable"].as_bool()
                != Some(
                    declaration
                        .get("cancellable")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false),
                )
        {
            return Err(format!("hook plan does not match declaration: {id}").into());
        }
        let helper_source = fs::read_to_string(
            root.join("mixins/vape-rs")
                .join(declaration["helper"].as_str().ok_or("missing helper")?),
        )?;
        let helper: syn::ItemFn = syn::parse_str(&helper_source)?;
        let planned_helper: syn::ItemFn = syn::parse_str(string(injection, "helper_source")?)?;
        if helper != planned_helper {
            return Err(format!("helper source differs from plan: {id}").into());
        }
        let name = &helper.sig.ident;
        let args = injection["bindings"]
            .as_array()
            .ok_or("missing bindings")?
            .iter()
            .map(|b| Ok(syn::parse_str::<syn::Expr>(string(b, "value")?)?))
            .collect::<Result<Vec<_>>>()?;
        let statement: syn::Stmt = if injection["cancellable"] == true {
            syn::parse2(
                quote!(if let ::core::ops::ControlFlow::Break(__horizon_mixins_return) = { #helper #name(#(#args),*) } { return __horizon_mixins_return; }),
            )?
        } else {
            syn::parse2(quote!({ #helper #name(#(#args),*) };))?
        };
        expected.push((
            string(injection, "file")?.to_owned(),
            string(injection, "target")?
                .rsplit("::")
                .next()
                .unwrap()
                .to_owned(),
            statement,
        ));
    }
    let files = expected
        .iter()
        .map(|(file, _, _)| file.clone())
        .chain(std::iter::once("crates/horizon-client/src/game.rs".into()))
        .collect::<std::collections::BTreeSet<_>>();
    for file in &files {
        if config["source_hashes"].get(file).is_none() {
            return Err(format!("hook source lacks a pinned hash: {file}").into());
        }
    }
    let mut removals = 0;
    for path in &files {
        let original = syn::parse_file(&fs::read_to_string(pristine.join(path))?)?;
        let mut modified = syn::parse_file(&fs::read_to_string(staged.join(path))?)?;
        let mut stripper = Strip {
            removed: 0,
            expected: expected
                .iter()
                .filter(|(file, _, _)| file == path)
                .map(|(_, name, stmt)| (name.clone(), stmt.clone()))
                .collect(),
        };
        if path == "crates/horizon-client/src/game.rs" {
            let payload = syn::parse_file(&fs::read_to_string(
                root.join("target/bundle/mixins/vape-rs/runtime.rs"),
            )?)?;
            let items = payload.items;
            let module: syn::File = syn::parse2(quote!(pub(crate) mod vape_rs { #(#items)* }))?;
            let module = syn::parse_file(&prettyplease::unparse(&module))?
                .items
                .remove(0);
            let index = modified
                .items
                .iter()
                .position(|item| item == &module)
                .ok_or("exact support module not found")?;
            modified.items.remove(index);
            removals += 1;
        }
        stripper.visit_file_mut(&mut modified);
        removals += stripper.removed;
        // Normalize token formatting inside macro token streams, as the transformer does.
        let original = syn::parse_file(&prettyplease::unparse(&original))?;
        let modified = syn::parse_file(&prettyplease::unparse(&modified))?;
        if original != modified {
            fs::write(
                root.join("target/audit-original.rs"),
                prettyplease::unparse(&original),
            )?;
            fs::write(
                root.join("target/audit-modified.rs"),
                prettyplease::unparse(&modified),
            )?;
            eprintln!("stripped {} hooks from {path}", stripper.removed);
            return Err(format!("unexpected transformed AST changes: {path}").into());
        }
    }
    if removals != injections.len() + 1 {
        return Err(format!(
            "expected {} hooks and one module, removed {removals}",
            injections.len()
        )
        .into());
    }
    for (path, expected_hash) in plan["inputs"].as_object().ok_or("missing plan inputs")? {
        if !files.contains(path)
            && expected_hash.as_str() != Some(hash(&fs::read(staged.join(path))?).as_str())
        {
            return Err(format!("unexpected staged file change: {path}").into());
        }
        if expected_hash.as_str() != Some(hash(&fs::read(pristine.join(path))?).as_str()) {
            return Err(format!("pristine export changed: {path}").into());
        }
    }
    println!(
        "audit: original AST restored after removing exactly {} HEAD hooks and 1 support module; other inputs unchanged",
        injections.len()
    );
    Ok(())
}

fn canonical_statement(statement: &syn::Stmt) -> String {
    let file: syn::File = syn::parse_quote!(fn audit() { #statement });
    prettyplease::unparse(&file)
}

struct Strip {
    removed: usize,
    expected: Vec<(String, syn::Stmt)>,
}
impl Strip {
    fn strip_head(&mut self, function: &syn::Ident, block: &mut syn::Block) {
        for (name, statement) in &self.expected {
            if function == name
                && block.stmts.first().is_some_and(|actual| {
                    canonical_statement(actual) == canonical_statement(statement)
                })
            {
                block.stmts.remove(0);
                self.removed += 1;
            }
        }
    }
}
impl VisitMut for Strip {
    fn visit_impl_item_fn_mut(&mut self, function: &mut syn::ImplItemFn) {
        self.strip_head(&function.sig.ident, &mut function.block);
    }
    fn visit_item_fn_mut(&mut self, function: &mut syn::ItemFn) {
        self.strip_head(&function.sig.ident, &mut function.block);
    }
}

fn host(root: &Path, mode: &str, horizon: &Path) -> Result<()> {
    let stage = stage(root, horizon)?;
    let log = root.join(format!("target/host-{mode}.log"));
    let output = fs::File::create(&log)?;
    let mut command = Command::new("cargo");
    command
        .current_dir(stage)
        .arg(mode)
        .args([
            "--locked",
            "--offline",
            "-p",
            "horizon-client",
            "--no-default-features",
            "--bin",
            "horizon-client",
        ])
        .arg("--target-dir")
        .arg(root.join("target/host-cargo"));
    if mode == "build" {
        command.args(["--profile", "fast"]);
    }
    if mode == "test" {
        command.args(["vape_rs::", "--", "--nocapture"]);
    }
    command
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output));
    println!("host {mode}: {}", log.display());
    checked(&mut command)
}

fn run() -> Result<()> {
    let root = root();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let action = args.first().map(String::as_str).unwrap_or("help");
    let horizon = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.parent().unwrap().join("horizon"));
    match action {
        "pack" => println!("{}", pack(&root)?.display()),
        "payload-test" => payload_test(&root)?,
        "stage" => println!("{}", stage(&root, &horizon)?.display()),
        "audit" => audit(&root)?,
        "check" | "build" | "test" => host(&root, action, &horizon)?,
        _ => println!(
            "cargo run -p xtask -- pack|payload-test|stage|audit|check|build|test [horizon-git-path]"
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_only_removes_exact_expected_hook_at_function_head() {
        let statement: syn::Stmt = syn::parse_quote!({ fn hook() {} hook() };);
        let mut file: syn::File = syn::parse_quote! {
            impl Game {
                fn tick() { { fn hook() {} hook() }; original(); }
                fn other() { { fn hook() {} hook() }; original(); }
            }
        };
        let mut strip = Strip {
            removed: 0,
            expected: vec![("tick".into(), statement)],
        };
        strip.visit_file_mut(&mut file);
        assert_eq!(strip.removed, 1);
        let expected: syn::File = syn::parse_quote! {
            impl Game {
                fn tick() { original(); }
                fn other() { { fn hook() {} hook() }; original(); }
            }
        };
        assert_eq!(file, expected);
    }

    #[test]
    fn audit_handles_cancellable_hook_on_free_function_without_erasing_original_returns() {
        let statement: syn::Stmt =
            syn::parse_quote!(if let ::core::ops::ControlFlow::Break(value) = hook(name) {
                return value;
            });
        let mut file: syn::File = syn::parse_quote! {
            fn key_code(name: &str) -> Option<KeyCode> {
                if let ::core::ops::ControlFlow::Break(value) = hook(name) { return value; }
                if name.is_empty() { return None; }
                original(name)
            }
        };
        let expected: syn::File = syn::parse_quote! {
            fn key_code(name: &str) -> Option<KeyCode> {
                if name.is_empty() { return None; }
                original(name)
            }
        };
        let mut strip = Strip {
            removed: 0,
            expected: vec![("key_code".into(), statement)],
        };
        strip.visit_file_mut(&mut file);
        assert_eq!(strip.removed, 1);
        assert_eq!(file, expected);
    }

    #[test]
    fn audit_rejects_changed_helper_or_reordered_call() {
        let statement: syn::Stmt = syn::parse_quote!({ fn hook() {} hook() };);
        let mut file: syn::File = syn::parse_quote! {
            impl Game {
                fn tick() { original(); { fn hook() {} hook() }; }
                fn changed() { { fn hook() { malicious(); } hook() }; original(); }
            }
        };
        let before = file.clone();
        let mut strip = Strip {
            removed: 0,
            expected: vec![
                ("tick".into(), statement.clone()),
                ("changed".into(), statement),
            ],
        };
        strip.visit_file_mut(&mut file);
        assert_eq!(strip.removed, 0);
        assert_eq!(file, before);
    }
}

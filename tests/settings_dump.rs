// Writes every setting definition (id, label, kind, default) as JSON to the
// path in VAPE_RS_SETTINGS_DUMP, for the settings-parity audit.
#[test]
fn dump_setting_definitions() {
    let Ok(path) = std::env::var("VAPE_RS_SETTINGS_DUMP") else {
        return;
    };
    let runtime = vape_rs::core::Runtime::default();
    let rows: Vec<serde_json::Value> = runtime
        .settings()
        .definitions()
        .map(|d| {
            serde_json::json!({
                "id": d.id, "label": d.label, "kind": format!("{:?}", d.kind),
                "default": d.default, "visible_if": d.visible_if.len(),
            })
        })
        .collect();
    std::fs::write(path, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
}

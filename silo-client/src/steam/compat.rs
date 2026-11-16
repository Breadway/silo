/// Updates the CompatToolMapping entry in Steam's config.vdf.
///
/// config.vdf is a text-format VDF file (not binary). Rather than pulling in
/// a full VDF parser/serde stack, we do targeted string surgery on the known
/// structure, which is stable across Steam versions.
///
/// The section we care about lives at:
///   "InstallConfigStore" > "Software" > "Valve" > "Steam" > "CompatToolMapping"
use std::path::Path;

/// Sets (or replaces) the compat tool for the given appid.
/// `proton_version` is the exact string Steam expects, e.g. "proton_9" or "GE-Proton9-20".
pub fn set_compat_tool(path: &Path, appid: u32, proton_version: &str) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let updated = upsert_mapping(&content, appid, proton_version)?;
    std::fs::write(path, updated)?;
    Ok(())
}

fn upsert_mapping(
    content: &str,
    appid: u32,
    proton_version: &str,
) -> anyhow::Result<String> {
    let appid_str = appid.to_string();
    let new_entry = format!(
        "\t\t\t\t\t\"{appid_str}\"\n\
         \t\t\t\t\t{{\n\
         \t\t\t\t\t\t\"name\"\t\t\"{proton_version}\"\n\
         \t\t\t\t\t\t\"config\"\t\t\"\"\n\
         \t\t\t\t\t\t\"Priority\"\t\t\"250\"\n\
         \t\t\t\t\t}}"
    );

    // If an entry for this appid already exists, replace it.
    if content.contains(&format!("\"{appid_str}\"")) {
        let updated = replace_existing_entry(content, &appid_str, &new_entry)?;
        return Ok(updated);
    }

    // Otherwise insert inside CompatToolMapping { ... }.
    let marker = "\"CompatToolMapping\"";
    let compat_start = content
        .find(marker)
        .ok_or_else(|| anyhow::anyhow!("CompatToolMapping section not found in config.vdf"))?;

    let brace_start = content[compat_start..]
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no opening brace after CompatToolMapping"))?
        + compat_start;

    let insert_pos = brace_start + 1;
    let mut out = content.to_owned();
    out.insert_str(insert_pos, &format!("\n{new_entry}"));
    Ok(out)
}

fn replace_existing_entry(
    content: &str,
    appid_str: &str,
    new_entry: &str,
) -> anyhow::Result<String> {
    // Find the line with `"<appid>"` and the matching braces block that follows.
    let key_pat = format!("\"{appid_str}\"");
    let key_pos = content
        .find(&key_pat)
        .ok_or_else(|| anyhow::anyhow!("appid entry disappeared"))?;

    let open_brace = content[key_pos..]
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no opening brace for appid entry"))?
        + key_pos;

    let mut depth = 0usize;
    let mut close_brace = open_brace;
    for (i, ch) in content[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close_brace = open_brace + i;
                    break;
                }
            }
            _ => {}
        }
    }

    // Find the start of the line holding the key.
    let line_start = content[..key_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);

    let mut out = content.to_owned();
    out.replace_range(line_start..=close_brace, new_entry);
    Ok(out)
}

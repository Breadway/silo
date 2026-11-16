/// Minimal binary VDF parser/writer for Steam's shortcuts.vdf.
///
/// Format reference:
///   https://developer.valvesoftware.com/wiki/KeyValues
///   CorporalQuesadilla/Steam-Shortcut-Manager
///
/// Type byte prefix:
///   0x00  nested object  (key\0, then items until 0x08)
///   0x01  string         (key\0, value\0)
///   0x02  int32 LE       (key\0, 4 bytes)
///   0x08  end of object  (no key follows)
use anyhow::Context;

const T_OBJECT: u8 = 0x00;
const T_STRING: u8 = 0x01;
const T_INT32: u8 = 0x02;
const T_END: u8 = 0x08;

/// One non-Steam game shortcut entry.
/// The `app_id` and `app_name`/`exe` strings must remain byte-identical with
/// whatever was used to compute the appid (see `super::appid::compute`), so
/// CompatToolMapping in config.vdf can reference the same appid.
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub app_id: u32,
    pub app_name: String,
    pub exe: String,
    pub start_dir: String,
    pub icon: String,
    pub launch_options: String,
}

// ── Public API ─────────────────────────────────────────────────────────────

pub fn parse(data: &[u8]) -> anyhow::Result<Vec<Shortcut>> {
    let mut pos = 0;
    let root = parse_object(data, &mut pos).context("parse shortcuts.vdf root")?;

    let shortcuts_items = root
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("shortcuts"))
        .and_then(|(_, v)| if let BvdfValue::Object(items) = v { Some(items) } else { None })
        .ok_or_else(|| anyhow::anyhow!("no 'shortcuts' object in file"))?;

    shortcuts_items
        .iter()
        .filter_map(|(_, v)| if let BvdfValue::Object(fields) = v { Some(fields) } else { None })
        .map(|fields| shortcut_from_fields(fields))
        .collect()
}

pub fn serialize(shortcuts: &[Shortcut]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_object_start(&mut buf, "shortcuts");
    for (i, sc) in shortcuts.iter().enumerate() {
        write_object_start(&mut buf, &i.to_string());
        write_int32(&mut buf, "appid", sc.app_id);
        write_string(&mut buf, "AppName", &sc.app_name);
        write_string(&mut buf, "Exe", &format!("\"{}\"", sc.exe));
        write_string(&mut buf, "StartDir", &sc.start_dir);
        write_string(&mut buf, "icon", &sc.icon);
        write_string(&mut buf, "ShortcutPath", "");
        write_string(&mut buf, "LaunchOptions", &sc.launch_options);
        write_int32(&mut buf, "IsHidden", 0);
        write_int32(&mut buf, "AllowDesktopConfig", 1);
        write_int32(&mut buf, "AllowOverlay", 1);
        write_int32(&mut buf, "OpenVR", 0);
        write_int32(&mut buf, "Devkit", 0);
        write_string(&mut buf, "DevkitGameID", "");
        write_int32(&mut buf, "DevkitOverrideAppID", 0);
        write_int32(&mut buf, "LastPlayTime", 0);
        write_string(&mut buf, "FlatpakAppID", "");
        write_object_start(&mut buf, "tags");
        buf.push(T_END); // end tags
        buf.push(T_END); // end this shortcut entry
    }
    buf.push(T_END); // end shortcuts
    buf.push(T_END); // end root
    buf
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn shortcut_from_fields(fields: &[(String, BvdfValue)]) -> anyhow::Result<Shortcut> {
    let str_field = |key: &str| -> String {
        fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .and_then(|(_, v)| if let BvdfValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_default()
    };
    let u32_field = |key: &str| -> u32 {
        fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .and_then(|(_, v)| if let BvdfValue::Int32(n) = v { Some(*n) } else { None })
            .unwrap_or(0)
    };

    Ok(Shortcut {
        app_id: u32_field("appid"),
        app_name: str_field("AppName"),
        exe: str_field("Exe").trim_matches('"').to_owned(),
        start_dir: str_field("StartDir"),
        icon: str_field("icon"),
        launch_options: str_field("LaunchOptions"),
    })
}

fn write_object_start(buf: &mut Vec<u8>, key: &str) {
    buf.push(T_OBJECT);
    write_cstring(buf, key);
}

fn write_string(buf: &mut Vec<u8>, key: &str, value: &str) {
    buf.push(T_STRING);
    write_cstring(buf, key);
    write_cstring(buf, value);
}

fn write_int32(buf: &mut Vec<u8>, key: &str, value: u32) {
    buf.push(T_INT32);
    write_cstring(buf, key);
    buf.extend_from_slice(&value.to_le_bytes());
}

fn write_cstring(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

// ── Internal VDF tree ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum BvdfValue {
    String(String),
    Int32(u32),
    Object(Vec<(String, BvdfValue)>),
}

fn parse_object(data: &[u8], pos: &mut usize) -> anyhow::Result<Vec<(String, BvdfValue)>> {
    let mut items = Vec::new();
    loop {
        if *pos >= data.len() {
            break;
        }
        let typ = data[*pos];
        if typ == T_END {
            *pos += 1;
            break;
        }
        *pos += 1;
        let key = read_cstring(data, pos)?;
        let val = match typ {
            T_OBJECT => BvdfValue::Object(parse_object(data, pos)?),
            T_STRING => BvdfValue::String(read_cstring(data, pos)?),
            T_INT32 => {
                anyhow::ensure!(*pos + 4 <= data.len(), "truncated int32 at pos {pos}");
                let n = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
                *pos += 4;
                BvdfValue::Int32(n)
            }
            other => anyhow::bail!("unknown type 0x{:02x} at pos {}", other, *pos - 1),
        };
        items.push((key, val));
    }
    Ok(items)
}

fn read_cstring(data: &[u8], pos: &mut usize) -> anyhow::Result<String> {
    let start = *pos;
    while *pos < data.len() && data[*pos] != 0 {
        *pos += 1;
    }
    anyhow::ensure!(*pos < data.len(), "unterminated string starting at pos {start}");
    let s = std::str::from_utf8(&data[start..*pos])
        .context("non-UTF-8 VDF string")?
        .to_owned();
    *pos += 1;
    Ok(s)
}

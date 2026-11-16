/// Computes the appid Steam assigns to a non-Steam shortcut.
///
/// Steam derives this from CRC32(exe_path + app_name), with the high bit set.
/// Both shortcuts.vdf (the appid field) and config.vdf (CompatToolMapping key)
/// must use this same value, derived from byte-identical strings.
pub fn compute(exe_path: &str, app_name: &str) -> u32 {
    // Steam wraps the exe path in double quotes before hashing.
    let input = format!("\"{}\"{}",  exe_path, app_name);
    let crc = crc32fast::hash(input.as_bytes());
    (crc | 0x80000000) & 0xffff_ffff
}

/// The "rungameid" value used in steam:// URIs (appid << 32 | 2).
pub fn to_run_game_id(appid: u32) -> u64 {
    ((appid as u64) << 32) | 2
}

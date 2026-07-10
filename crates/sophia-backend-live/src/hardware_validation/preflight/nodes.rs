pub(crate) fn is_primary_card_node_entry(entry: &std::fs::DirEntry) -> bool {
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
        return false;
    };
    if !is_primary_card_node_name(name) {
        return false;
    }
    entry
        .file_type()
        .map(|file_type| is_drm_card_node_file_type(&file_type))
        .unwrap_or(false)
}

fn is_primary_card_node_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("card") else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(unix)]
fn is_drm_card_node_file_type(file_type: &std::fs::FileType) -> bool {
    use std::os::unix::fs::FileTypeExt;

    file_type.is_char_device()
}

#[cfg(not(unix))]
fn is_drm_card_node_file_type(_file_type: &std::fs::FileType) -> bool {
    false
}

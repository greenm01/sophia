const SUBMIT_PREFIX: &str = "sophia_runtime_rendered_scanout_submit";
const RETIRE_PREFIX: &str = "sophia_runtime_rendered_scanout_retire";
const CLEANUP_PREFIX: &str = "sophia_runtime_rendered_scanout_cleanup";

const CLEAN_SUBMIT_FIELDS: &[(&str, &str)] = &[
    ("schema", "6"),
    ("status", "SubmittedWaitingForPageFlip"),
    ("scanout_target", "Ready"),
    ("output_size", "MatchingReducedSize"),
    ("target", "Ready"),
    ("target_size", "MatchingReducedSize"),
    ("export", "Exported"),
    ("scanout_buffer", "Ready"),
    ("buffer_format", "SupportedBufferFormat"),
    ("buffer_modifier", "SupportedBufferModifier"),
    ("buffer_planes", "SupportedBufferPlanes"),
    ("properties", "Discovered"),
    ("format_table", "KnownFormatTableState"),
    ("resources", "Created"),
    ("framebuffer", "CreatedFramebuffer"),
    ("request", "Built"),
    ("submit", "SubmittedWaitingForPageFlip"),
    ("request_scope", "PageFlip"),
    ("commit_page_flip_event", "true"),
    ("commit_nonblocking", "true"),
    ("commit_allow_modeset", "false"),
    ("commit_test_only", "false"),
    ("commit_submit", "Submitted"),
    ("runtime_scanout_state", "Submitted"),
    ("in_flight", "true"),
    ("in_flight_ticks", "0"),
    ("cleanup_pending", "false"),
];

const CLEAN_RETIRE_FIELDS: &[(&str, &str)] = &[
    ("schema", "1"),
    ("status", "RetiredAfterPageFlip"),
    ("destroy", "Destroyed"),
    ("runtime_scanout_state", "Retired"),
    ("in_flight", "false"),
    ("in_flight_ticks", "0"),
    ("cleanup_pending", "false"),
];

pub fn runtime_rendered_scanout_evidence_is_clean(lines: &[String]) -> bool {
    let mut submit_seen = false;
    let mut retire_seen = false;

    for line in lines {
        if line.starts_with(CLEANUP_PREFIX) {
            return false;
        }
        if parse_exact_evidence_line(line, SUBMIT_PREFIX, CLEAN_SUBMIT_FIELDS) {
            if submit_seen {
                return false;
            }
            submit_seen = true;
            continue;
        }
        if parse_exact_evidence_line(line, RETIRE_PREFIX, CLEAN_RETIRE_FIELDS) {
            if retire_seen {
                return false;
            }
            retire_seen = true;
            continue;
        }
        return false;
    }

    submit_seen && retire_seen
}

fn parse_exact_evidence_line(line: &str, prefix: &str, required: &[(&str, &str)]) -> bool {
    let Some(fields) = line
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix(' '))
    else {
        return false;
    };
    let mut seen = 0u128;
    let mut seen_count = 0usize;
    let mut output_size = None;
    let mut target_size = None;

    for field in fields.split_ascii_whitespace() {
        let Some((key, value)) = field.split_once('=') else {
            return false;
        };
        let Some(index) = required.iter().position(|(required_key, required_value)| {
            key == *required_key && evidence_field_matches(key, value, required_value)
        }) else {
            return false;
        };
        let bit = 1u128 << index;
        if seen & bit != 0 {
            return false;
        }
        seen |= bit;
        seen_count += 1;

        if key == "output_size" {
            output_size = Some(value);
        }
        if key == "target_size" {
            target_size = Some(value);
        }
    }

    if seen_count != required.len() {
        return false;
    }
    if required.iter().any(|(key, _)| *key == "output_size")
        && required.iter().any(|(key, _)| *key == "target_size")
        && output_size != target_size
    {
        return false;
    }

    true
}

fn evidence_field_matches(key: &str, value: &str, required_value: &str) -> bool {
    if matches!(key, "output_size" | "target_size") && required_value == "MatchingReducedSize" {
        return reduced_size_is_valid(value);
    }
    if key == "framebuffer" && required_value == "CreatedFramebuffer" {
        return matches!(
            value,
            "CreatedWithAddFb2" | "CreatedWithAddFb2Modifiers" | "CreatedWithLegacyAddFb"
        );
    }
    if key == "buffer_format" && required_value == "SupportedBufferFormat" {
        return matches!(value, "Xrgb8888" | "Argb8888");
    }
    if key == "buffer_modifier" && required_value == "SupportedBufferModifier" {
        return matches!(value, "Implicit" | "Linear" | "NonLinear");
    }
    if key == "buffer_planes" && required_value == "SupportedBufferPlanes" {
        return matches!(value, "Single" | "Multiple");
    }
    if key == "format_table" && required_value == "KnownFormatTableState" {
        return matches!(value, "Present" | "Missing");
    }

    value == required_value
}

fn reduced_size_is_valid(value: &str) -> bool {
    let Some((width, height)) = value.split_once('x') else {
        return false;
    };
    matches!(
        (width.parse::<u32>(), height.parse::<u32>()),
        (Ok(width), Ok(height)) if width > 0 && height > 0
    )
}

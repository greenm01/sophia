use sophia_x_authority::{XResourceId, select_x_close_target};

fn window(raw: u64) -> XResourceId {
    XResourceId::new(raw, 1)
}

#[test]
fn exact_protocol_window_wins_for_multi_toplevel_client() {
    let exact = window(0x200002);
    let decision = select_x_close_target(exact, &[], &[(window(0x200001), true), (exact, true)]);
    assert_eq!(decision.window, exact);
    assert!(decision.exact_advertises_delete);
    assert!(!decision.fallback_used);
    assert_eq!(decision.protocol_window_count, 2);
}

#[test]
fn unique_protocol_window_is_the_only_allowed_fallback() {
    let exact = window(0x200002);
    let unique = select_x_close_target(exact, &[], &[(window(0x200001), true)]);
    assert_eq!(unique.window, window(0x200001));
    assert!(unique.fallback_used);

    let ambiguous = select_x_close_target(
        exact,
        &[],
        &[(window(0x200001), true), (window(0x200003), true)],
    );
    assert_eq!(ambiguous.window, exact);
    assert!(!ambiguous.fallback_used);
    assert_eq!(ambiguous.protocol_window_count, 2);
}

#[test]
fn no_protocol_window_keeps_the_exact_target() {
    let exact = window(0x200002);
    let decision = select_x_close_target(exact, &[], &[]);
    assert_eq!(decision.window, exact);
    assert!(!decision.exact_advertises_delete);
    assert!(!decision.fallback_used);
    assert_eq!(decision.protocol_window_count, 0);
}

#[test]
fn nearest_protocol_ancestor_resolves_multi_toplevel_children() {
    let exact = window(0x200004);
    let parent = window(0x200003);
    let decision = select_x_close_target(
        exact,
        &[parent, window(0x200001)],
        &[(window(0x200001), true), (parent, true)],
    );
    assert_eq!(decision.window, parent);
    assert!(decision.fallback_used);
    assert_eq!(decision.protocol_window_count, 2);
}

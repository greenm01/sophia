use crate::XResourceId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XCloseTargetDecision {
    pub window: XResourceId,
    pub exact_advertises_delete: bool,
    pub fallback_used: bool,
    pub protocol_window_count: usize,
}

pub fn select_x_close_target(
    exact: XResourceId,
    ancestors: &[XResourceId],
    protocol_windows: &[(XResourceId, bool)],
) -> XCloseTargetDecision {
    let eligible: Vec<_> = protocol_windows
        .iter()
        .filter_map(|(window, advertises_delete)| advertises_delete.then_some(*window))
        .collect();
    let exact_advertises_delete = eligible.contains(&exact);
    let ancestor = (!exact_advertises_delete)
        .then(|| {
            ancestors
                .iter()
                .copied()
                .find(|window| eligible.contains(window))
        })
        .flatten();
    let fallback =
        ancestor.or_else(|| (!exact_advertises_delete && eligible.len() == 1).then(|| eligible[0]));
    XCloseTargetDecision {
        window: fallback.unwrap_or(exact),
        exact_advertises_delete,
        fallback_used: fallback.is_some(),
        protocol_window_count: eligible.len(),
    }
}

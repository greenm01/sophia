use sophia_backend_live::LiveProductionPresentationAdapter;
use sophia_engine::{ProductionPresentationAdapter, ProductionRetirement};
use sophia_protocol::CommittedSurfaceState;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn live_adapter_keeps_frame_and_retirement_inside_ordered_callbacks() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let compose_calls = Rc::clone(&calls);
    let submit_calls = Rc::clone(&calls);
    let feedback_calls = Rc::clone(&calls);
    let retire_calls = Rc::clone(&calls);
    let pending = Rc::new(RefCell::new(Vec::new()));
    let submitted = Rc::clone(&pending);
    let retired = Rc::clone(&pending);
    let mut adapter = LiveProductionPresentationAdapter::new(
        move |cycle, committed: &[CommittedSurfaceState]| {
            compose_calls.borrow_mut().push(("compose", cycle));
            Ok::<_, &str>(committed.len())
        },
        move |cycle, frame| {
            submit_calls.borrow_mut().push(("submit", cycle));
            submitted.borrow_mut().push(ProductionRetirement {
                cycle,
                retirement: frame + 1,
            });
            Ok::<_, &str>(frame)
        },
        move || {
            retire_calls.borrow_mut().push(("retire", 7));
            Ok::<_, &str>(retired.borrow_mut().drain(..).collect())
        },
        move |cycle, retirement| {
            feedback_calls.borrow_mut().push(("feedback", cycle));
            Ok::<_, &str>(retirement + 1)
        },
    );

    let frame = adapter.compose(7, &[]).unwrap();
    let submission = adapter.submit_frame(7, frame).unwrap();
    let retirement = adapter.poll_retirements().unwrap().pop().unwrap();
    let evidence = adapter
        .route_protocol_feedback(retirement.cycle, retirement.retirement)
        .unwrap();

    assert_eq!(submission, 0);
    assert_eq!(evidence, 2);
    assert_eq!(
        *calls.borrow(),
        [
            ("compose", 7),
            ("submit", 7),
            ("retire", 7),
            ("feedback", 7),
        ]
    );
}

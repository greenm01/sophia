#[test]
fn query_observes_untriggered_then_triggered_xshmfence() {
    let fd = sophia_xshmfence::allocate().unwrap();
    assert_eq!(sophia_xshmfence::query(&fd), Ok(false));
    sophia_xshmfence::trigger(&fd).unwrap();
    assert_eq!(sophia_xshmfence::query(&fd), Ok(true));
}

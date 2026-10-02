use opengeometry_test_support::stored::compared_when;

#[test]
fn stored_results_are_compared_unless_ci_is_true() {
    assert!(compared_when(None));
    assert!(compared_when(Some("false")));
    assert!(compared_when(Some("1")));
    assert!(compared_when(Some("")));
    assert!(!compared_when(Some("true")));
}

use super::nodes;
use crate::world_graph::ErrorCode;

fn node_list(count: usize, id: &str) -> String {
    serde_json::to_string(&vec![id; count]).unwrap()
}

#[test]
fn export_node_list_is_exempt_from_the_params_cap() {
    let json = node_list(2_000, "00000000-0000-4000-8000-000000000000");
    assert_eq!(json.len(), 78_001);
    assert_eq!(nodes(&json).unwrap().len(), 2_000);
}

#[test]
fn export_node_list_rejects_more_than_100000_ids() {
    assert_eq!(nodes(&node_list(100_000, "a")).unwrap().len(), 100_000);
    assert_eq!(
        nodes(&node_list(100_001, "a")).unwrap_err().error_code(),
        ErrorCode::LimitExceeded
    );
}

#[test]
fn export_node_id_over_1024_chars_is_invalid_parameter() {
    assert!(nodes(&node_list(1, &"é".repeat(1024))).is_ok());
    assert_eq!(
        nodes(&node_list(1, &"a".repeat(1025)))
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidParameter
    );
}

#[test]
fn malformed_export_node_list_is_invalid_parameter() {
    for json in ["[\"a\"", "[\"a\"] x", "{\"a\":1}", "[1]", ""] {
        let error = nodes(json).unwrap_err();
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
        assert!(error.to_string().contains("invalid params JSON: "));
    }
}

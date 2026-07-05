use crate::domain::DomainTimestamp;

#[test]
fn timestamp_constructor_validates_utc_shape() {
    let timestamp = DomainTimestamp::from_utc_string("2026-07-05T01:02:03Z")
        .expect("valid UTC timestamp shape should parse");

    assert_eq!(timestamp.as_str(), "2026-07-05T01:02:03Z");
    assert!(DomainTimestamp::from_utc_string("not-a-time").is_err());
}

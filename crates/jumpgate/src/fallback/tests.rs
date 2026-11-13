use super::*;
use crate::error::Reason;

#[test]
fn a_fallback_value_is_accepted() {
    let value: FallbackValue = "local-placeholder-token".parse().unwrap();
    assert_eq!(value.as_str(), "local-placeholder-token");
}

#[test]
fn an_empty_fallback_value_is_accepted() {
    assert!("".parse::<FallbackValue>().is_ok());
}

#[test]
fn a_fallback_value_with_a_nul_character_is_refused_with_the_fallback_code() {
    let error = "local\0token".parse::<FallbackValue>().unwrap_err();
    assert_eq!(error.reason(), Reason::Fallback);
    assert_eq!(error.code().to_string(), "jumpgate::record::fallback");
}

#[test]
fn a_fallback_value_of_4096_bytes_is_accepted_and_one_of_4097_is_refused() {
    assert!("a".repeat(4096).parse::<FallbackValue>().is_ok());
    assert_eq!(
        "a".repeat(4097)
            .parse::<FallbackValue>()
            .unwrap_err()
            .reason(),
        Reason::Fallback
    );
}

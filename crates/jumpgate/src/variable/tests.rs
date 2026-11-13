use super::*;
use crate::error::Reason;

#[test]
fn a_variable_name_of_letters_digits_and_underscores_is_accepted() {
    let name: VariableName = "AUTH_API_URL_2".parse().unwrap();
    assert_eq!(name.as_str(), "AUTH_API_URL_2");
}

#[test]
fn a_lowercase_variable_name_keeps_its_case() {
    assert_eq!(
        "https_proxy".parse::<VariableName>().unwrap().as_str(),
        "https_proxy"
    );
}

#[test]
fn a_variable_name_that_starts_with_an_underscore_is_accepted() {
    assert!("_PRIVATE".parse::<VariableName>().is_ok());
}

#[test]
fn a_variable_name_that_starts_with_a_digit_is_refused_with_the_variable_code() {
    let error = "1PORT".parse::<VariableName>().unwrap_err();
    assert_eq!(error.reason(), Reason::Variable);
    assert_eq!(error.code().to_string(), "jumpgate::record::variable");
}

#[test]
fn an_empty_variable_name_is_refused() {
    assert_eq!(
        "".parse::<VariableName>().unwrap_err().reason(),
        Reason::Variable
    );
}

#[test]
fn a_variable_name_with_a_hyphen_is_refused() {
    assert_eq!(
        "AUTH-URL".parse::<VariableName>().unwrap_err().reason(),
        Reason::Variable
    );
}

#[test]
fn a_variable_name_of_128_bytes_is_accepted_and_one_of_129_is_refused() {
    assert!("A".repeat(128).parse::<VariableName>().is_ok());
    assert_eq!(
        "A".repeat(129)
            .parse::<VariableName>()
            .unwrap_err()
            .reason(),
        Reason::Variable
    );
}

#[test]
fn a_refused_variable_name_is_never_quoted() {
    let message = "s3cr3t=value"
        .parse::<VariableName>()
        .unwrap_err()
        .to_string();
    assert!(!message.contains("s3cr3t"), "{message}");
}

use super::*;
use crate::error::Reason;

/// Returns the reason `text` is refused as `T`, failing when it is accepted.
fn refusal<T: std::str::FromStr<Err = RecordError> + std::fmt::Debug>(text: &str) -> RecordError {
    text.parse::<T>().expect_err("the name is refused")
}

#[test]
fn an_app_name_of_lowercase_letters_digits_hyphens_and_underscores_is_accepted() {
    let name: AppName = "shelf-api_2".parse().unwrap();
    assert_eq!(name.as_str(), "shelf-api_2");
    assert_eq!(name.to_string(), "shelf-api_2");
}

#[test]
fn an_app_name_that_starts_with_a_digit_is_accepted() {
    assert!("2shelf".parse::<AppName>().is_ok());
}

#[test]
fn an_empty_app_name_is_refused_with_the_name_code() {
    let error = refusal::<AppName>("");
    assert_eq!(error.reason(), Reason::Name);
    assert_eq!(error.code().to_string(), "jumpgate::record::name");
}

#[test]
fn an_app_name_of_64_bytes_is_accepted() {
    assert!("a".repeat(64).parse::<AppName>().is_ok());
}

#[test]
fn an_app_name_of_65_bytes_is_refused() {
    let error = refusal::<AppName>(&"a".repeat(65));
    assert_eq!(error.reason(), Reason::Name);
    assert!(error.to_string().contains("64"), "{error}");
}

#[test]
fn an_app_name_that_starts_with_a_hyphen_is_refused() {
    assert_eq!(refusal::<AppName>("-shelf").reason(), Reason::Name);
}

#[test]
fn an_app_name_that_starts_with_an_underscore_is_refused() {
    assert_eq!(refusal::<AppName>("_shelf").reason(), Reason::Name);
}

#[test]
fn an_app_name_with_an_uppercase_letter_is_refused() {
    assert_eq!(refusal::<AppName>("Shelf").reason(), Reason::Name);
}

#[test]
fn an_app_name_with_a_slash_is_refused() {
    assert_eq!(refusal::<AppName>("shelf/api").reason(), Reason::Name);
}

#[test]
fn an_app_name_with_a_dot_is_refused() {
    assert_eq!(refusal::<AppName>("..").reason(), Reason::Name);
    assert_eq!(refusal::<AppName>("shelf.api").reason(), Reason::Name);
}

#[test]
fn an_app_name_with_a_character_outside_ascii_is_refused() {
    assert_eq!(refusal::<AppName>("shélf").reason(), Reason::Name);
}

#[test]
fn a_refused_name_message_names_the_kind_of_name_and_never_quotes_the_text() {
    let error = refusal::<NeedName>("Secret-Value-123");
    let message = error.to_string();
    assert!(message.contains("a need name"), "{message}");
    assert!(!message.contains("Secret-Value-123"), "{message}");
}

#[test]
fn a_unit_name_with_an_uppercase_letter_is_accepted_as_mxv_accepts_it() {
    let name: UnitName = "Worker".parse().unwrap();
    assert_eq!(name.as_str(), "Worker");
    assert!("HTTP".parse::<PortName>().is_ok());
}

#[test]
fn a_unit_name_with_a_dot_is_refused() {
    assert_eq!(refusal::<UnitName>("api.v2").reason(), Reason::Name);
}

#[test]
fn a_unit_name_that_starts_with_a_hyphen_is_refused() {
    assert_eq!(refusal::<PortName>("-http").reason(), Reason::Name);
}

#[test]
fn every_lowercase_name_type_refuses_an_uppercase_letter_and_accepts_its_lowercase_twin() {
    fn pair<T: std::str::FromStr<Err = RecordError> + std::fmt::Debug>() {
        assert!("local-dev".parse::<T>().is_ok());
        assert_eq!(refusal::<T>("Local").reason(), Reason::Name);
    }
    pair::<AppName>();
    pair::<NeedName>();
    pair::<TargetName>();
    pair::<ProviderName>();
    pair::<OfferName>();
    pair::<ProfileName>();
    pair::<BoardId>();
    pair::<Projection>();
    pair::<Audience>();
}

use super::*;
use crate::error::Reason;

fn refused(text: &str) -> RecordError {
    text.parse::<SecretReference>()
        .expect_err("the reference is refused")
}

#[test]
fn a_three_part_one_password_reference_is_accepted() {
    let reference: SecretReference = "op://shelf-dev/api-token/credential".parse().unwrap();
    assert_eq!(reference.as_str(), "op://shelf-dev/api-token/credential");
}

#[test]
fn a_reference_whose_parts_hold_spaces_dots_and_underscores_is_accepted() {
    assert!(
        "op://Shelf Dev/api token.v2/one_time"
            .parse::<SecretReference>()
            .is_ok()
    );
}

#[test]
fn a_bare_value_is_refused_with_the_secret_reference_code() {
    let error = refused("hunter2");
    assert_eq!(error.reason(), Reason::SecretReference);
    assert_eq!(
        error.code().to_string(),
        "jumpgate::record::secret_reference"
    );
}

#[test]
fn a_refusal_never_quotes_the_refused_text() {
    for text in [
        "sk_live_abcdef123456",
        "op://vault/item",
        "op://vault/item/field/extra/s3cr3t",
        "OP://vault/item/s3cr3t",
    ] {
        let message = refused(text).to_string();
        assert!(!message.contains(text), "{message}");
        assert!(!message.contains("s3cr3t"), "{message}");
        assert!(!message.contains("abcdef"), "{message}");
    }
}

#[test]
fn a_reference_with_two_parts_is_refused() {
    assert_eq!(refused("op://vault/item").reason(), Reason::SecretReference);
}

#[test]
fn a_reference_with_a_section_part_is_accepted() {
    let reference: SecretReference = "op://vault/item/section/field".parse().unwrap();
    assert_eq!(reference.as_str(), "op://vault/item/section/field");
}

#[test]
fn a_reference_with_five_parts_is_refused() {
    assert_eq!(
        refused("op://vault/item/section/field/extra").reason(),
        Reason::SecretReference
    );
}

#[test]
fn a_reference_whose_parts_hold_apostrophes_and_letters_outside_ascii_is_accepted() {
    for text in [
        "op://Team's vault/api token/credential",
        "op://Équipe/clé d'API/mot de passe",
        "op://vault/item/field?attribute=otp",
        "op:// vault/item /field",
    ] {
        let reference: SecretReference = text.parse().unwrap();
        assert_eq!(reference.as_str(), text);
    }
}

#[test]
fn a_reference_with_another_scheme_is_refused() {
    assert_eq!(refused("vault://a/b/c").reason(), Reason::SecretReference);
    assert_eq!(refused("OP://a/b/c").reason(), Reason::SecretReference);
}

#[test]
fn a_reference_with_an_empty_part_is_refused() {
    assert_eq!(
        refused("op://vault//field").reason(),
        Reason::SecretReference
    );
}

#[test]
fn a_reference_whose_part_is_a_dot_or_two_is_refused() {
    assert_eq!(
        refused("op://vault/../field").reason(),
        Reason::SecretReference
    );
    assert_eq!(
        refused("op://./item/field").reason(),
        Reason::SecretReference
    );
}

#[test]
fn a_reference_with_a_control_character_is_refused() {
    assert_eq!(
        refused("op://vault/item/fie\nld").reason(),
        Reason::SecretReference
    );
}

#[test]
fn a_reference_of_any_length_is_accepted_as_starmap_accepts_it() {
    let reference = format!("op://{}/item/field", "a".repeat(8192));
    assert!(reference.parse::<SecretReference>().is_ok());
}

#[test]
fn a_reference_whose_part_is_a_dot_or_two_in_the_section_or_field_is_refused() {
    assert_eq!(
        refused("op://vault/item/./field").reason(),
        Reason::SecretReference
    );
    assert_eq!(
        refused("op://vault/item/..").reason(),
        Reason::SecretReference
    );
}

#[test]
fn a_reference_whose_part_holds_three_dots_is_accepted() {
    assert!("op://vault/.../field".parse::<SecretReference>().is_ok());
}

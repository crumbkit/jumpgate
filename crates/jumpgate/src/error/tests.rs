use super::*;

#[test]
fn a_code_is_written_with_the_tool_its_area_and_its_name() {
    assert_eq!(
        Code::new("record", "secret_reference").to_string(),
        "jumpgate::record::secret_reference"
    );
}

#[test]
fn every_reason_has_a_distinct_record_code() {
    let codes: Vec<String> = Reason::ALL
        .iter()
        .map(|reason| reason.code().to_string())
        .collect();
    for (index, code) in codes.iter().enumerate() {
        assert!(code.starts_with("jumpgate::record::"), "{code}");
        assert!(
            !codes.iter().take(index).any(|earlier| earlier == code),
            "{code} repeats"
        );
    }
}

#[test]
fn an_error_at_no_field_shows_only_its_message() {
    let error = RecordError::new(Reason::Syntax, "the text is not JSON");
    assert_eq!(error.field(), None);
    assert_eq!(error.to_string(), "the text is not JSON");
}

#[test]
fn an_error_placed_in_nested_fields_names_the_whole_field_path() {
    let error = RecordError::new(Reason::Name, "a unit name is empty")
        .at_index(2)
        .within("consumers");
    assert_eq!(error.field(), Some("consumers[2]"));
    assert_eq!(error.to_string(), "consumers[2]: a unit name is empty");
    let nested = RecordError::new(Reason::Repository, "bad")
        .within("repo")
        .within("declared_in");
    assert_eq!(nested.field(), Some("declared_in.repo"));
    let in_list = RecordError::new(Reason::Name, "bad")
        .within("profile")
        .at_index(0)
        .within("when");
    assert_eq!(in_list.field(), Some("when[0].profile"));
}

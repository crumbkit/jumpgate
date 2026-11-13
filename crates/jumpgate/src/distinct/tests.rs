use super::*;
use crate::name::UnitName;

fn units(names: &[&str]) -> Vec<UnitName> {
    names.iter().map(|name| name.parse().unwrap()).collect()
}

#[test]
fn a_list_without_repeats_keeps_its_order() {
    let list: Distinct<UnitName> = Distinct::new(units(&["worker", "api"])).unwrap();
    let names: Vec<&str> = list.iter().map(UnitName::as_str).collect();
    assert_eq!(names, ["worker", "api"]);
}

#[test]
fn a_list_that_repeats_an_entry_is_refused_at_the_repeat_with_the_duplicate_code() {
    let error = Distinct::<UnitName>::new(units(&["api", "worker", "api"])).unwrap_err();
    assert_eq!(error.reason(), Reason::Duplicate);
    assert_eq!(error.code().to_string(), "jumpgate::record::duplicate");
    assert_eq!(error.field(), Some("[2]"));
}

#[test]
fn an_empty_list_is_accepted_when_it_needs_no_entry() {
    assert!(Distinct::<UnitName>::new(Vec::new()).unwrap().is_empty());
}

#[test]
fn an_empty_list_is_refused_with_the_empty_code_when_it_needs_an_entry() {
    let error = Distinct::<UnitName, 1>::new(Vec::new()).unwrap_err();
    assert_eq!(error.reason(), Reason::Empty);
    assert_eq!(error.code().to_string(), "jumpgate::record::empty");
}

#[test]
fn a_list_with_one_entry_is_accepted_when_it_needs_one() {
    assert_eq!(
        Distinct::<UnitName, 1>::new(units(&["api"])).unwrap().len(),
        1
    );
}

#[test]
fn a_json_list_reads_each_entry_and_places_a_refusal_at_its_index() {
    let value = serde_json::json!(["api", "-bad"]);
    let error = Distinct::<UnitName>::from_json(&value).unwrap_err();
    assert_eq!(error.reason(), Reason::Name);
    assert_eq!(error.field(), Some("[1]"));
}

#[test]
fn a_json_value_that_is_not_a_list_is_refused_with_the_shape_code() {
    let value = serde_json::json!("api");
    let error = Distinct::<UnitName>::from_json(&value).unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
}

#[test]
fn a_json_list_of_distinct_entries_is_accepted() {
    let value = serde_json::json!(["api", "worker"]);
    assert_eq!(Distinct::<UnitName>::from_json(&value).unwrap().len(), 2);
}

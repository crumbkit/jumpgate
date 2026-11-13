use super::*;

fn refused(text: &str) -> Reason {
    text.parse::<RecordPath>()
        .expect_err("the path is refused")
        .reason()
}

#[test]
fn a_need_path_reads_its_app_and_name_and_writes_the_same_text() {
    let path: NeedPath = "/jumpgate/needs/shelf/api_token".parse().unwrap();
    assert_eq!(path.app().as_str(), "shelf");
    assert_eq!(path.name().as_str(), "api_token");
    assert_eq!(path.to_string(), "/jumpgate/needs/shelf/api_token");
}

#[test]
fn an_offer_path_reads_its_provider_kind_provider_and_name() {
    let path: OfferPath = "/jumpgate/offers/secret-store/one-password/shelf-api-token"
        .parse()
        .unwrap();
    assert_eq!(path.provider_kind(), ProviderKind::SecretStore);
    assert_eq!(path.provider().as_str(), "one-password");
    assert_eq!(path.name().as_str(), "shelf-api-token");
    assert_eq!(
        path.to_string(),
        "/jumpgate/offers/secret-store/one-password/shelf-api-token"
    );
}

#[test]
fn a_binding_path_and_a_target_path_read_back_from_their_text() {
    let binding: BindingPath = "/jumpgate/bindings/local/shelf/api_token".parse().unwrap();
    assert_eq!(binding.target().as_str(), "local");
    assert_eq!(binding.app().as_str(), "shelf");
    assert_eq!(binding.need().as_str(), "api_token");
    assert_eq!(
        binding.to_string(),
        "/jumpgate/bindings/local/shelf/api_token"
    );
    let target: TargetPath = "/jumpgate/targets/local".parse().unwrap();
    assert_eq!(target.name().as_str(), "local");
}

#[test]
fn a_record_path_reads_each_kind_of_path() {
    for text in [
        "/jumpgate/needs/shelf/api_token",
        "/jumpgate/offers/app/auth/auth-api",
        "/jumpgate/bindings/local/shelf/api_token",
        "/jumpgate/targets/local",
    ] {
        let path: RecordPath = text.parse().unwrap();
        assert_eq!(path.to_string(), text);
    }
}

#[test]
fn a_need_path_with_a_dot_dot_segment_is_refused_with_the_record_path_code() {
    let error = "/jumpgate/needs/../api_token"
        .parse::<NeedPath>()
        .unwrap_err();
    assert_eq!(error.reason(), Reason::RecordPath);
    assert_eq!(error.code().to_string(), "jumpgate::record::record_path");
}

#[test]
fn a_need_path_with_an_extra_segment_is_refused() {
    assert_eq!(
        refused("/jumpgate/needs/shelf/api/token"),
        Reason::RecordPath
    );
}

#[test]
fn a_need_path_with_a_missing_segment_is_refused() {
    assert_eq!(refused("/jumpgate/needs/shelf"), Reason::RecordPath);
    assert_eq!(refused("/jumpgate/needs/shelf/"), Reason::RecordPath);
}

#[test]
fn a_path_with_an_uppercase_segment_is_refused() {
    assert_eq!(
        refused("/jumpgate/needs/Shelf/api_token"),
        Reason::RecordPath
    );
}

#[test]
fn an_offer_path_with_an_unknown_provider_kind_is_refused() {
    assert_eq!(
        refused("/jumpgate/offers/vault/auth/auth-api"),
        Reason::RecordPath
    );
}

#[test]
fn a_path_outside_jumpgate_is_refused() {
    assert_eq!(refused("/elsewhere/shelf/api_token"), Reason::RecordPath);
    assert_eq!(
        refused("jumpgate/needs/shelf/api_token"),
        Reason::RecordPath
    );
}

#[test]
fn a_need_path_does_not_parse_as_an_offer_path() {
    let error = "/jumpgate/needs/shelf/api_token"
        .parse::<OfferPath>()
        .unwrap_err();
    assert_eq!(error.reason(), Reason::RecordPath);
}

#[test]
fn a_refused_path_is_never_quoted() {
    let message = "/jumpgate/needs/shelf/S3CR3T"
        .parse::<NeedPath>()
        .unwrap_err()
        .to_string();
    assert!(!message.contains("S3CR3T"), "{message}");
}

#[test]
fn a_unit_path_reads_its_app_and_unit_and_writes_the_same_text() {
    let path: UnitPath = "/jumpgate/units/shelf/api".parse().unwrap();
    assert_eq!(path.app().as_str(), "shelf");
    assert_eq!(path.unit().as_str(), "api");
    assert_eq!(path.to_string(), "/jumpgate/units/shelf/api");
    let any: RecordPath = "/jumpgate/units/shelf/api".parse().unwrap();
    assert_eq!(any, RecordPath::Unit(path));
}

#[test]
fn a_unit_path_whose_unit_has_an_uppercase_letter_is_refused() {
    assert_eq!(refused("/jumpgate/units/shelf/Api"), Reason::RecordPath);
}

#[test]
fn building_a_unit_path_with_an_uppercase_unit_is_refused_with_the_name_code() {
    let error = UnitPath::new("shelf".parse().unwrap(), "Api".parse().unwrap()).unwrap_err();
    assert_eq!(error.reason(), Reason::Name);
}

#[test]
fn building_a_unit_path_with_a_lowercase_unit_is_accepted() {
    let path = UnitPath::new("shelf".parse().unwrap(), "api-1".parse().unwrap()).unwrap();
    assert_eq!(path.to_string(), "/jumpgate/units/shelf/api-1");
}

#[test]
fn a_unit_path_with_a_missing_segment_is_refused() {
    assert_eq!(refused("/jumpgate/units/shelf"), Reason::RecordPath);
}

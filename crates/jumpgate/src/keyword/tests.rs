use super::*;
use crate::error::Reason;

#[test]
fn every_need_kind_reads_back_from_its_word() {
    for kind in NeedKind::ALL {
        assert_eq!(kind.as_str().parse::<NeedKind>().unwrap(), *kind);
    }
    assert_eq!(
        "data-access".parse::<NeedKind>().unwrap(),
        NeedKind::DataAccess
    );
}

#[test]
fn an_unknown_need_kind_is_refused_with_the_keyword_code_and_the_words_it_takes() {
    let error = "password".parse::<NeedKind>().unwrap_err();
    assert_eq!(error.reason(), Reason::Keyword);
    assert_eq!(error.code().to_string(), "jumpgate::record::keyword");
    let message = error.to_string();
    assert!(
        message.contains("secret, address, value, data-access, capability, app"),
        "{message}"
    );
    assert!(!message.contains("password"), "{message}");
}

#[test]
fn a_keyword_in_another_case_is_refused() {
    assert_eq!(
        "Secret".parse::<NeedKind>().unwrap_err().reason(),
        Reason::Keyword
    );
}

#[test]
fn every_provider_kind_reads_back_from_its_word() {
    for kind in ProviderKind::ALL {
        assert_eq!(kind.as_str().parse::<ProviderKind>().unwrap(), *kind);
    }
    assert_eq!(
        "identity-broker".parse::<ProviderKind>().unwrap(),
        ProviderKind::IdentityBroker
    );
}

#[test]
fn an_unknown_provider_kind_is_refused() {
    assert_eq!(
        "vault".parse::<ProviderKind>().unwrap_err().reason(),
        Reason::Keyword
    );
}

#[test]
fn a_scope_reads_from_its_word_and_an_unknown_one_is_refused() {
    assert_eq!("app-instance".parse::<Scope>().unwrap(), Scope::AppInstance);
    assert_eq!("workspace".parse::<Scope>().unwrap(), Scope::Workspace);
    assert_eq!(
        "global".parse::<Scope>().unwrap_err().reason(),
        Reason::Keyword
    );
}

#[test]
fn a_target_kind_reads_from_its_word_and_an_unknown_one_is_refused() {
    assert_eq!("local".parse::<TargetKind>().unwrap(), TargetKind::Local);
    assert_eq!("cloud".parse::<TargetKind>().unwrap(), TargetKind::Cloud);
    assert_eq!(
        "staging".parse::<TargetKind>().unwrap_err().reason(),
        Reason::Keyword
    );
}

#[test]
fn each_service_kind_reads_back_from_its_word_and_has_its_endpoint_variable() {
    for (kind, word, variable) in [
        (
            ServiceKind::Dynamodb,
            "dynamodb",
            "AWS_ENDPOINT_URL_DYNAMODB",
        ),
        (ServiceKind::S3, "s3", "AWS_ENDPOINT_URL_S3"),
        (ServiceKind::Sqs, "sqs", "AWS_ENDPOINT_URL_SQS"),
        (ServiceKind::Kms, "kms", "AWS_ENDPOINT_URL_KMS"),
    ] {
        assert_eq!(word.parse::<ServiceKind>().unwrap(), kind);
        assert_eq!(kind.as_str(), word);
        assert_eq!(kind.endpoint_variable(), variable);
    }
    assert_eq!(ServiceKind::ALL.len(), 4);
}

#[test]
fn an_unknown_service_kind_is_refused() {
    let error = "redis".parse::<ServiceKind>().unwrap_err();
    assert_eq!(error.reason(), Reason::Keyword);
}

use std::collections::BTreeMap;

use super::*;
use crate::keyword::UnitKind;
use crate::name::UnitName;
use crate::record::{DeliversAs, Reference, Request, WhenUnbound};

/// A checkout that holds files in a map, and can fail to read one.
#[derive(Default)]
struct MapCheckout {
    files: BTreeMap<String, String>,
    unreadable: Vec<String>,
}

impl MapCheckout {
    fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_owned(), text.to_owned());
        self
    }
}

impl Checkout for MapCheckout {
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
        if self.unreadable.iter().any(|name| name == path.as_str()) {
            return Err(CheckoutError::new("permission denied"));
        }
        Ok(self.files.get(path.as_str()).cloned())
    }
}

const SHELF_MXV: &str = include_str!("../../../../fixtures/stack/shelf/mxv.toml");
const SHELF_STARMAP: &str = include_str!("../../../../fixtures/stack/shelf/starmap.toml");
const AUTH_MXV: &str = include_str!("../../../../fixtures/stack/auth/mxv.toml");
const LEDGER_MXV: &str = include_str!("../../../../fixtures/stack/ledger/mxv.toml");

fn id(text: &str) -> RepoId {
    text.parse().unwrap()
}

fn shelf() -> MapCheckout {
    MapCheckout::default()
        .with("mxv.toml", SHELF_MXV)
        .with("starmap.toml", SHELF_STARMAP)
}

fn auth() -> MapCheckout {
    MapCheckout::default().with("mxv.toml", AUTH_MXV)
}

fn ledger() -> MapCheckout {
    MapCheckout::default().with("mxv.toml", LEDGER_MXV)
}

/// Reads one repository `repo` with `checkout`.
fn read_one(repo: &str, checkout: &MapCheckout) -> Declarations {
    let repo = id(repo);
    read(&[Repository::new(&repo, checkout)])
}

/// Reads the three fixture repositories.
fn read_stack() -> Declarations {
    let (shelf_id, auth_id, ledger_id) = (
        id("github.com/acme/shelf"),
        id("github.com/acme/auth"),
        id("github.com/acme/ledger"),
    );
    let (shelf, auth, ledger) = (shelf(), auth(), ledger());
    read(&[
        Repository::new(&shelf_id, &shelf),
        Repository::new(&auth_id, &auth),
        Repository::new(&ledger_id, &ledger),
    ])
}

fn codes(declarations: &Declarations) -> Vec<String> {
    declarations
        .refusals()
        .iter()
        .map(|refusal| refusal.code().to_string())
        .collect()
}

fn need<'a>(declarations: &'a Declarations, path: &str) -> &'a Need {
    declarations
        .needs()
        .iter()
        .find(|need| need.path().to_string() == path)
        .unwrap_or_else(|| panic!("no need at {path}"))
}

fn offer_names(declarations: &Declarations) -> Vec<String> {
    declarations
        .offers()
        .iter()
        .map(|offer| offer.path().to_string())
        .collect()
}

#[test]
fn the_fixture_stack_is_read_without_a_refusal() {
    let declarations = read_stack();
    assert_eq!(declarations.refusals(), &[]);
    assert_eq!(declarations.unread(), &[]);
}

#[test]
fn an_app_provides_an_offer_for_each_port_route_and_unit() {
    let declarations = read_stack();
    let mut names = offer_names(&declarations);
    names.sort();
    assert_eq!(
        names,
        [
            "/jumpgate/offers/app/auth/api",
            "/jumpgate/offers/app/auth/api-route",
            "/jumpgate/offers/app/auth/server-unit",
            "/jumpgate/offers/app/ledger/store-unit",
            "/jumpgate/offers/app/ledger/worker-unit",
            "/jumpgate/offers/app/shelf/db",
            "/jumpgate/offers/app/shelf/db-unit",
            "/jumpgate/offers/app/shelf/http",
            "/jumpgate/offers/app/shelf/web-unit",
            "/jumpgate/offers/secret-store/one-password/shelf-dev-api-token-credential",
        ]
    );
}

#[test]
fn a_port_and_a_route_of_the_same_name_make_two_offers_of_different_references() {
    let declarations = read_stack();
    let port = declarations
        .offers()
        .iter()
        .find(|offer| offer.path().to_string() == "/jumpgate/offers/app/auth/api")
        .unwrap();
    let route = declarations
        .offers()
        .iter()
        .find(|offer| offer.path().to_string() == "/jumpgate/offers/app/auth/api-route")
        .unwrap();
    assert!(matches!(port.reference(), Reference::Port(_)));
    assert!(matches!(route.reference(), Reference::Route(_)));
    assert_eq!(port.kind(), NeedKind::Address);
    assert_eq!(route.kind(), NeedKind::Address);
}

#[test]
fn a_unit_has_a_record_with_its_order_and_its_ports() {
    let declarations = read_stack();
    let web = declarations
        .units()
        .iter()
        .find(|unit| unit.path().to_string() == "/jumpgate/units/shelf/web")
        .unwrap();
    assert_eq!(web.kind(), UnitKind::Process);
    assert_eq!(web.repo().as_str(), "github.com/acme/shelf");
    let after: Vec<&str> = web.starts_after().iter().map(UnitName::as_str).collect();
    assert_eq!(after, ["db"]);
    let ports: Vec<&str> = web.ports().iter().map(|port| port.as_str()).collect();
    assert_eq!(ports, ["http"]);
    let db = declarations
        .units()
        .iter()
        .find(|unit| unit.path().to_string() == "/jumpgate/units/shelf/db")
        .unwrap();
    assert_eq!(db.kind(), UnitKind::Container);
    let ports: Vec<&str> = db.ports().iter().map(|port| port.as_str()).collect();
    assert_eq!(ports, ["db"]);
}

#[test]
fn a_want_becomes_a_need_of_every_unit_with_the_kind_of_the_offer_it_names() {
    let declarations = read_stack();
    let want = need(&declarations, "/jumpgate/needs/shelf/auth-api");
    assert_eq!(want.kind(), NeedKind::Address);
    let mut consumers: Vec<&str> = want.consumers().iter().map(UnitName::as_str).collect();
    consumers.sort_unstable();
    assert_eq!(consumers, ["db", "web"]);
    assert_eq!(want.declared_in().file().as_str(), "mxv.toml");
    assert_eq!(want.declared_in().repo().as_str(), "github.com/acme/shelf");
    assert!(!want.optional());
    assert_eq!(want.when_unbound(), &WhenUnbound::Fail);
    let Some(Request::Want(request)) = want.requests() else {
        panic!("the need requests a want");
    };
    assert_eq!(request.repo().as_str(), "github.com/acme/auth");
    assert_eq!(request.provides().as_str(), "api");
    let DeliversAs::Variables(variables) = want.delivers_as() else {
        panic!("a want delivers as a variable");
    };
    assert_eq!(variables.as_slice()[0].as_str(), "AUTH_API_URL");
}

#[test]
fn a_want_of_a_unit_takes_the_kind_app() {
    let mxv = AUTH_MXV.to_owned()
        + "\n[wants.shelf-web]\nrepo = \"github.com/acme/shelf\"\nprovides = \"web-unit\"\nas = \"SHELF_UNIT\"\n";
    let (shelf_id, auth_id) = (id("github.com/acme/shelf"), id("github.com/acme/auth"));
    let (shelf, auth) = (shelf(), MapCheckout::default().with("mxv.toml", &mxv));
    let declarations = read(&[
        Repository::new(&shelf_id, &shelf),
        Repository::new(&auth_id, &auth),
    ]);
    assert_eq!(declarations.refusals(), &[]);
    assert_eq!(
        need(&declarations, "/jumpgate/needs/auth/shelf-web").kind(),
        NeedKind::App
    );
}

#[test]
fn an_optional_want_is_omitted_when_unbound() {
    let declarations = read_stack();
    let want = need(&declarations, "/jumpgate/needs/ledger/billing-api");
    assert!(want.optional());
    assert_eq!(want.when_unbound(), &WhenUnbound::Omit);
}

#[test]
fn a_want_of_a_repository_that_is_not_read_keeps_the_kind_address() {
    let declarations = read_stack();
    let want = need(&declarations, "/jumpgate/needs/ledger/billing-api");
    assert_eq!(want.kind(), NeedKind::Address);
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn a_want_of_a_repository_that_was_read_and_lacks_the_name_is_refused() {
    let mxv = LEDGER_MXV.replace("provides = \"http\"", "provides = \"nothing\"");
    let (shelf_id, ledger_id) = (id("github.com/acme/shelf"), id("github.com/acme/ledger"));
    let (shelf, ledger) = (shelf(), MapCheckout::default().with("mxv.toml", &mxv));
    let declarations = read(&[
        Repository::new(&shelf_id, &shelf),
        Repository::new(&ledger_id, &ledger),
    ]);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::provider"]);
    let refusal = &declarations.refusals()[0];
    assert_eq!(refusal.file().as_str(), "mxv.toml");
    assert_eq!(refusal.line(), Some(8));
    assert!(
        declarations
            .needs()
            .iter()
            .all(|need| need.name().as_str() != "shelf-http")
    );
}

#[test]
fn a_secret_becomes_a_need_of_the_processes_that_read_its_starmap_file() {
    let declarations = read_stack();
    let token = need(&declarations, "/jumpgate/needs/shelf/api-token");
    assert_eq!(token.kind(), NeedKind::Secret);
    let consumers: Vec<&str> = token.consumers().iter().map(UnitName::as_str).collect();
    assert_eq!(consumers, ["web"]);
    assert_eq!(token.declared_in().file().as_str(), "starmap.toml");
    let DeliversAs::Variables(variables) = token.delivers_as() else {
        panic!("a secret delivers as a variable");
    };
    assert_eq!(variables.as_slice()[0].as_str(), "API_TOKEN");
}

#[test]
fn a_secret_required_in_production_fails_when_unbound_and_an_optional_one_is_omitted() {
    let declarations = read_stack();
    let required = need(&declarations, "/jumpgate/needs/shelf/api-token");
    assert!(!required.optional());
    assert_eq!(required.when_unbound(), &WhenUnbound::Fail);
    let optional = need(&declarations, "/jumpgate/needs/shelf/session-key");
    assert!(optional.optional());
    assert_eq!(optional.when_unbound(), &WhenUnbound::Omit);
}

#[test]
fn a_bound_secret_requests_the_offer_derived_from_its_reference() {
    let declarations = read_stack();
    let token = need(&declarations, "/jumpgate/needs/shelf/api-token");
    let Some(Request::Bind(path)) = token.requests() else {
        panic!("the secret requests a bind");
    };
    assert_eq!(
        path.to_string(),
        "/jumpgate/offers/secret-store/one-password/shelf-dev-api-token-credential"
    );
    let offer = declarations
        .offers()
        .iter()
        .find(|offer| offer.path() == *path)
        .unwrap();
    assert_eq!(offer.kind(), NeedKind::Secret);
    let Reference::Secret(reference) = offer.reference() else {
        panic!("the offer holds a secret reference");
    };
    assert_eq!(reference.as_str(), "op://shelf-dev/api-token/credential");
}

#[test]
fn an_unbound_secret_requests_nothing() {
    let declarations = read_stack();
    let key = need(&declarations, "/jumpgate/needs/shelf/session-key");
    assert_eq!(key.requests(), None);
}

#[test]
fn two_binds_of_one_reference_make_one_offer() {
    let starmap = format!(
        "{SHELF_STARMAP}\n[variables.OLD_TOKEN]\npurpose = \"The old name of the token.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\nbind = {{ one-password = \"op://shelf-dev/api-token/credential\" }}\n"
    );
    let checkout = shelf().with("starmap.toml", &starmap);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(declarations.refusals(), &[]);
    let secrets = declarations
        .offers()
        .iter()
        .filter(|offer| offer.kind() == NeedKind::Secret)
        .count();
    assert_eq!(secrets, 1);
    let old = need(&declarations, "/jumpgate/needs/shelf/old-token");
    let new = need(&declarations, "/jumpgate/needs/shelf/api-token");
    assert_eq!(old.requests(), new.requests());
}

#[test]
fn two_references_that_make_one_slug_get_different_names_with_a_digest() {
    let starmap = "[variables.A]\npurpose = \"One.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\nbind = { one-password = \"op://v/item.x/f\" }\n\n[variables.B]\npurpose = \"Two.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\nbind = { one-password = \"op://v/item-x/f\" }\n";
    let checkout = shelf().with("starmap.toml", starmap);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    let names: Vec<String> = offer_names(&declarations)
        .into_iter()
        .filter(|name| name.contains("one-password"))
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(
        names.iter().all(|name| name.contains("v-item-x-f-")),
        "{names:?}"
    );
    assert_ne!(names[0], names[1]);
}

#[test]
fn a_starmap_file_that_no_process_reads_is_reported_and_gives_no_need() {
    let mxv = "[processes.web]\nargv = [\"./web\"]\ncwd = \"web\"\n";
    let checkout = MapCheckout::default()
        .with("mxv.toml", mxv)
        .with("starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(declarations.needs(), &[]);
    assert_eq!(declarations.unread().len(), 1);
    assert_eq!(declarations.unread()[0].file().as_str(), "starmap.toml");
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn a_process_with_a_cwd_reads_the_starmap_file_in_that_directory() {
    let mxv = "[processes.web]\nargv = [\"./web\"]\ncwd = \"./services/web/\"\n";
    let checkout = MapCheckout::default()
        .with("mxv.toml", mxv)
        .with("services/web/starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(declarations.refusals(), &[]);
    let token = need(&declarations, "/jumpgate/needs/shelf/api-token");
    assert_eq!(
        token.declared_in().file().as_str(),
        "services/web/starmap.toml"
    );
    assert_eq!(declarations.unread(), &[]);
}

#[test]
fn a_cwd_outside_the_checkout_is_refused_and_the_unit_is_still_read() {
    let mxv = "[processes.web]\nargv = [\"./web\"]\ncwd = \"../elsewhere\"\n";
    let checkout = MapCheckout::default().with("mxv.toml", mxv);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::directory"]);
    assert_eq!(declarations.units().len(), 1);
    assert_eq!(declarations.refusals()[0].line(), Some(1));
}

#[test]
fn a_cwd_inside_the_checkout_is_not_refused() {
    let mxv = "[processes.web]\nargv = [\"./web\"]\ncwd = \"inner/dir\"\n";
    let checkout = MapCheckout::default().with("mxv.toml", mxv);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn a_repository_without_mxv_toml_is_refused_and_reads_nothing_else() {
    let checkout = MapCheckout::default().with("starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::missing"]);
    assert_eq!(declarations.needs(), &[]);
}

#[test]
fn a_file_that_cannot_be_read_is_refused() {
    let mut checkout = shelf();
    checkout.unreadable.push("mxv.toml".to_owned());
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::unreadable"]);
}

#[test]
fn a_file_over_the_size_limit_is_refused_and_one_at_the_limit_is_read() {
    let padding = |bytes: usize| format!("{}\n# {}", "ports = []", "x".repeat(bytes));
    let base = padding(0).len();
    let at_limit = MapCheckout::default().with("mxv.toml", &padding(MAX_DECLARATION_BYTES - base));
    assert_eq!(read_one("github.com/acme/shelf", &at_limit).refusals(), &[]);
    let over = MapCheckout::default().with("mxv.toml", &padding(MAX_DECLARATION_BYTES - base + 1));
    assert_eq!(
        codes(&read_one("github.com/acme/shelf", &over)),
        ["jumpgate::declaration::too_large"]
    );
}

#[test]
fn an_mxv_file_that_is_not_toml_is_refused_at_its_line() {
    let checkout = MapCheckout::default().with("mxv.toml", "ports = [\"a\"]\n\nthis is not toml\n");
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::syntax"]);
    assert_eq!(declarations.refusals()[0].line(), Some(3));
}

#[test]
fn an_mxv_file_with_a_table_of_another_tool_and_keys_of_its_own_is_read() {
    let mxv = format!(
        "version = 1\ndisposable = [\"target/\"]\n\n[phase]\nanything = true\n\n{AUTH_MXV}"
    );
    let checkout = MapCheckout::default().with("mxv.toml", &mxv);
    assert_eq!(read_one("github.com/acme/auth", &checkout).refusals(), &[]);
}

#[test]
fn a_repository_whose_name_is_not_an_app_name_is_refused() {
    let checkout = auth();
    let declarations = read_one("github.com/acme/auth.rs", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::name"]);
    let accepted = read_one("github.com/acme/auth-rs", &checkout);
    assert_eq!(accepted.refusals(), &[]);
}

#[test]
fn a_port_name_with_an_uppercase_letter_is_refused_with_its_line_and_a_lowercase_one_is_read() {
    let refused = MapCheckout::default().with("mxv.toml", "\nports = [\"Api\"]\n");
    let declarations = read_one("github.com/acme/auth", &refused);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::name"]);
    assert_eq!(declarations.refusals()[0].line(), Some(2));
    let accepted = MapCheckout::default().with("mxv.toml", "\nports = [\"api\"]\n");
    assert_eq!(read_one("github.com/acme/auth", &accepted).refusals(), &[]);
}

#[test]
fn a_unit_name_with_an_uppercase_letter_is_refused_and_a_lowercase_one_is_read() {
    let refused = MapCheckout::default().with("mxv.toml", "[processes.Api]\nargv = [\"./a\"]\n");
    let declarations = read_one("github.com/acme/auth", &refused);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::name"]);
    assert_eq!(declarations.units(), &[]);
    let accepted = MapCheckout::default().with("mxv.toml", "[processes.api]\nargv = [\"./a\"]\n");
    assert_eq!(read_one("github.com/acme/auth", &accepted).units().len(), 1);
}

#[test]
fn a_port_named_like_a_route_clashes_and_is_refused_once() {
    let mxv = "ports = [\"api\", \"api-route\"]\nroutes = [\"api\"]\n";
    let checkout = MapCheckout::default().with("mxv.toml", mxv);
    let declarations = read_one("github.com/acme/auth", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::clash"]);
    let without = MapCheckout::default().with(
        "mxv.toml",
        "ports = [\"api\", \"api-extra\"]\nroutes = [\"api\"]\n",
    );
    assert_eq!(read_one("github.com/acme/auth", &without).refusals(), &[]);
}

/// Returns an mxv file with one process and the want `want`, a table body.
fn mxv_with_want(want: &str) -> MapCheckout {
    MapCheckout::default().with(
        "mxv.toml",
        &format!("[processes.web]\nargv = [\"./web\"]\n\n[wants.auth-api]\n{want}\n"),
    )
}

const GOOD_WANT: &str = "repo = \"github.com/acme/auth\"\nprovides = \"api\"\nas = \"AUTH_URL\"";

#[test]
fn a_want_with_every_key_valid_is_read() {
    let declarations = read_one("github.com/acme/shelf", &mxv_with_want(GOOD_WANT));
    assert_eq!(declarations.refusals(), &[]);
    assert_eq!(declarations.needs().len(), 1);
}

#[test]
fn a_want_that_is_not_a_table_is_refused_at_its_line() {
    let checkout = MapCheckout::default().with(
        "mxv.toml",
        "[processes.web]\nargv = [\"./web\"]\n\n[wants]\nauth-api = 3\n",
    );
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::want"]);
    assert_eq!(declarations.refusals()[0].line(), Some(5));
}

#[test]
fn a_want_with_a_key_it_does_not_have_is_refused() {
    let want = format!("{GOOD_WANT}\nkind = \"address\"");
    let declarations = read_one("github.com/acme/shelf", &mxv_with_want(&want));
    assert_eq!(codes(&declarations), ["jumpgate::declaration::want"]);
}

#[test]
fn a_want_without_a_required_key_is_refused_for_each_key() {
    for missing in ["repo", "provides", "as"] {
        let want: Vec<&str> = GOOD_WANT
            .lines()
            .filter(|line| !line.starts_with(missing))
            .collect();
        let declarations = read_one("github.com/acme/shelf", &mxv_with_want(&want.join("\n")));
        assert_eq!(
            codes(&declarations),
            ["jumpgate::declaration::want"],
            "{missing}"
        );
    }
}

#[test]
fn a_want_with_a_bad_value_is_refused_for_each_key() {
    for (key, bad) in [
        ("repo", "repo = \"auth\""),
        ("repo", "repo = 3"),
        ("provides", "provides = \"Api\""),
        ("as", "as = \"1BAD\""),
        ("optional", "optional = \"yes\""),
    ] {
        let want: Vec<String> = GOOD_WANT
            .lines()
            .filter(|line| !line.starts_with(key))
            .map(str::to_owned)
            .chain([bad.to_owned()])
            .collect();
        let declarations = read_one("github.com/acme/shelf", &mxv_with_want(&want.join("\n")));
        assert_eq!(
            codes(&declarations),
            ["jumpgate::declaration::want"],
            "{bad}"
        );
    }
}

#[test]
fn a_want_named_with_an_uppercase_letter_is_refused() {
    let checkout = MapCheckout::default().with(
        "mxv.toml",
        &format!("[processes.web]\nargv = [\"./web\"]\n\n[wants.Auth]\n{GOOD_WANT}\n"),
    );
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::want"]);
}

#[test]
fn a_refused_want_leaves_the_other_wants_and_the_secrets_read() {
    let mxv = format!(
        "[processes.web]\nargv = [\"./web\"]\n\n[wants.bad]\nrepo = 3\n\n[wants.good]\n{GOOD_WANT}\n"
    );
    let checkout = MapCheckout::default()
        .with("mxv.toml", &mxv)
        .with("starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::want"]);
    let names: Vec<&str> = declarations
        .needs()
        .iter()
        .map(|need| need.name().as_str())
        .collect();
    assert_eq!(names, ["api-token", "good", "session-key"]);
}

#[test]
fn a_refused_repository_leaves_the_other_repositories_read() {
    let (bad_id, auth_id) = (id("github.com/acme/shelf"), id("github.com/acme/auth"));
    let (bad, auth) = (MapCheckout::default(), auth());
    let declarations = read(&[
        Repository::new(&bad_id, &bad),
        Repository::new(&auth_id, &auth),
    ]);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::missing"]);
    assert_eq!(declarations.units().len(), 1);
}

#[test]
fn a_want_in_an_app_with_no_unit_is_refused() {
    let checkout =
        MapCheckout::default().with("mxv.toml", &format!("[wants.auth-api]\n{GOOD_WANT}\n"));
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::consumers"]);
}

#[test]
fn a_want_named_like_a_secret_clashes() {
    let want = GOOD_WANT;
    let checkout = MapCheckout::default()
        .with(
            "mxv.toml",
            &format!("[processes.web]\nargv = [\"./web\"]\n\n[wants.api-token]\n{want}\n"),
        )
        .with("starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::clash"]);
}

#[test]
fn two_variables_that_make_one_need_name_clash() {
    let starmap = "[variables.API_TOKEN]\npurpose = \"One.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\n\n[variables.api_token]\npurpose = \"Two.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\n";
    let checkout = shelf().with("starmap.toml", starmap);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::clash"]);
}

#[test]
fn a_starmap_file_that_does_not_parse_is_refused_at_its_line_and_the_wants_still_read() {
    let starmap = "[variables.A]\npurpose = \"One.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\nfoo = 1\n";
    let checkout = shelf().with("starmap.toml", starmap);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::starmap"]);
    assert_eq!(declarations.refusals()[0].file().as_str(), "starmap.toml");
    assert!(declarations.refusals()[0].line().is_some());
    assert_eq!(declarations.needs().len(), 1);
}

#[test]
fn a_secret_variable_that_starts_with_an_underscore_is_refused_as_a_need_name() {
    let starmap = "[variables._A]\npurpose = \"One.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\n";
    let declarations = read_one(
        "github.com/acme/shelf",
        &shelf().with("starmap.toml", starmap),
    );
    assert_eq!(codes(&declarations), ["jumpgate::declaration::starmap"]);
    let ok = starmap.replace("_A", "A_");
    let declarations = read_one("github.com/acme/shelf", &shelf().with("starmap.toml", &ok));
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn a_setting_variable_gives_no_need() {
    let starmap = "[variables.LEVEL]\npurpose = \"The log level.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"shown\"\nsource = \"setting\"\n";
    let declarations = read_one(
        "github.com/acme/shelf",
        &shelf().with("starmap.toml", starmap),
    );
    let secrets = declarations
        .needs()
        .iter()
        .filter(|need| need.kind() == NeedKind::Secret)
        .count();
    assert_eq!(secrets, 0);
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn no_record_holds_a_secret_value() {
    let declarations = read_stack();
    let text: String = declarations
        .needs()
        .iter()
        .map(|need| serde_json::to_string(need).unwrap())
        .chain(
            declarations
                .offers()
                .iter()
                .map(|offer| serde_json::to_string(offer).unwrap()),
        )
        .collect();
    assert!(text.contains("op://shelf-dev/api-token/credential"));
    assert!(!text.contains("The token shelf sends"));
}

#[test]
fn a_refusal_never_quotes_a_value_of_the_file() {
    let checkout = MapCheckout::default().with(
        "mxv.toml",
        "[processes.web]\nargv = [\"./web\"]\n\n[wants.auth-api]\nrepo = \"s3cr3t-token\"\nprovides = \"api\"\nas = \"X\"\n",
    );
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(declarations.refusals().len(), 1);
    assert!(!declarations.refusals()[0].message().contains("s3cr3t"));
}

#[test]
fn every_reason_has_a_declaration_code() {
    for reason in Reason::ALL {
        let code = reason.code().to_string();
        assert!(code.starts_with("jumpgate::declaration::"), "{code}");
    }
    assert_eq!(Reason::ALL.len(), 11);
}

#[test]
fn the_two_declaration_file_names_are_valid_repository_paths() {
    assert_eq!(RepoFilePath::known(MXV_FILE), MXV_FILE.parse().unwrap());
    assert_eq!(
        RepoFilePath::known(STARMAP_FILE),
        STARMAP_FILE.parse().unwrap()
    );
}

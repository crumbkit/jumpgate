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
    root: Option<String>,
}

impl MapCheckout {
    fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_owned(), text.to_owned());
        self
    }
}

impl MapCheckout {
    /// Gives the checkout the directory `root`.
    fn at(mut self, root: &str) -> Self {
        self.root = Some(root.to_owned());
        self
    }
}

impl Checkout for MapCheckout {
    fn root(&self) -> Result<&str, CheckoutError> {
        self.root
            .as_deref()
            .ok_or_else(|| CheckoutError::new("the checkout is not a directory"))
    }

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

/// The provide that the fixture's bound secret derives, whose name ends in the
/// digest of its reference.
const SHELF_TOKEN_PROVIDE: &str =
    "/jumpgate/provides/secret-store/one-password/shelf-dev-api-token-credential-76e290dd";

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
fn read_one(repo: &str, checkout: &dyn Checkout) -> Declarations {
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

fn provide_names(declarations: &Declarations) -> Vec<String> {
    declarations
        .provides()
        .iter()
        .map(|provide| provide.path().to_string())
        .collect()
}

#[test]
fn the_fixture_stack_is_read_without_a_refusal() {
    let declarations = read_stack();
    assert_eq!(declarations.refusals(), &[]);
    assert_eq!(declarations.unread(), &[]);
}

#[test]
fn an_app_provides_a_provide_for_each_port_route_and_unit() {
    let declarations = read_stack();
    let mut names = provide_names(&declarations);
    names.sort();
    assert_eq!(
        names,
        [
            "/jumpgate/provides/app/auth/api",
            "/jumpgate/provides/app/auth/server",
            "/jumpgate/provides/app/auth/site",
            "/jumpgate/provides/app/ledger/store",
            "/jumpgate/provides/app/ledger/worker",
            "/jumpgate/provides/app/shelf/db-port",
            "/jumpgate/provides/app/shelf/db-unit",
            "/jumpgate/provides/app/shelf/http",
            "/jumpgate/provides/app/shelf/web",
            SHELF_TOKEN_PROVIDE,
        ]
    );
}

#[test]
fn a_port_and_a_unit_of_the_same_name_make_two_provides_named_with_their_kinds() {
    let declarations = read_stack();
    let port = declarations
        .provides()
        .iter()
        .find(|provide| provide.path().to_string() == "/jumpgate/provides/app/shelf/db-port")
        .unwrap();
    let route = declarations
        .provides()
        .iter()
        .find(|provide| provide.path().to_string() == "/jumpgate/provides/app/shelf/db-unit")
        .unwrap();
    assert!(matches!(port.reference(), Reference::Port(_)));
    assert!(matches!(route.reference(), Reference::Unit(_)));
    assert_eq!(port.kind(), NeedKind::Address);
    assert_eq!(route.kind(), NeedKind::App);
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
fn a_need_becomes_a_need_of_every_unit_with_the_kind_of_the_provide_it_names() {
    let declarations = read_stack();
    let need = need(&declarations, "/jumpgate/needs/shelf/auth-api");
    assert_eq!(need.kind(), NeedKind::Address);
    let mut consumers: Vec<String> = need.consumers().iter().map(ToString::to_string).collect();
    consumers.sort_unstable();
    assert_eq!(consumers, ["db", "web"]);
    assert_eq!(need.declared_in().file().as_str(), "mxv.toml");
    assert_eq!(need.declared_in().repo().as_str(), "github.com/acme/shelf");
    assert!(!need.optional());
    assert_eq!(need.when_unbound(), &WhenUnbound::Fail);
    let Some(Request::Need(request)) = need.requests() else {
        panic!("the need requests a need");
    };
    assert_eq!(request.repo().as_str(), "github.com/acme/auth");
    assert_eq!(request.provide().as_str(), "api");
    let DeliversAs::Variables(variables) = need.delivers_as() else {
        panic!("a need delivers as a variable");
    };
    assert_eq!(variables.as_slice()[0].as_str(), "AUTH_API_URL");
}

#[test]
fn a_need_of_a_unit_takes_the_kind_app() {
    let mxv = AUTH_MXV.to_owned()
        + "\n[needs.shelf-web]\nrepo = \"github.com/acme/shelf\"\nfrom = \"web\"\nas = \"SHELF_UNIT\"\n";
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
fn an_optional_need_is_omitted_when_unbound() {
    let declarations = read_stack();
    let need = need(&declarations, "/jumpgate/needs/ledger/billing-api");
    assert_eq!(need.when_unbound(), &WhenUnbound::Omit);
    assert_eq!(need.when_unbound(), &WhenUnbound::Omit);
}

#[test]
fn a_need_of_a_repository_that_is_not_read_keeps_the_kind_address() {
    let declarations = read_stack();
    let need = need(&declarations, "/jumpgate/needs/ledger/billing-api");
    assert_eq!(need.kind(), NeedKind::Address);
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn a_need_of_a_repository_that_was_read_and_lacks_the_name_is_refused() {
    let mxv = LEDGER_MXV.replace("from = \"http\"", "from = \"nothing\"");
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
fn a_need_of_a_repository_that_does_not_end_in_an_app_name_is_refused() {
    let mxv = LEDGER_MXV.replace("github.com/acme/shelf", "github.com/acme/shelf.rs");
    let declarations = read_one(
        "github.com/acme/ledger",
        &MapCheckout::default().with("mxv.toml", &mxv),
    );
    assert_eq!(codes(&declarations), ["jumpgate::declaration::provider"]);
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
    let consumers: Vec<String> = token.consumers().iter().map(ToString::to_string).collect();
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
fn a_bound_secret_requests_the_provide_derived_from_its_reference() {
    let declarations = read_stack();
    let token = need(&declarations, "/jumpgate/needs/shelf/api-token");
    let Some(Request::Bind(path)) = token.requests() else {
        panic!("the secret requests a bind");
    };
    assert_eq!(path.to_string(), SHELF_TOKEN_PROVIDE);
    let provide = declarations
        .provides()
        .iter()
        .find(|provide| provide.path() == *path)
        .unwrap();
    assert_eq!(provide.kind(), NeedKind::Secret);
    let Reference::Secret(reference) = provide.reference() else {
        panic!("the provide holds a secret reference");
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
fn two_binds_of_one_reference_make_one_provide() {
    let starmap = format!(
        "{SHELF_STARMAP}\n[variables.OLD_TOKEN]\npurpose = \"The old name of the token.\"\ntype = \"string\"\nunset = \"optional\"\nshow = \"withheld\"\nsource = \"secret\"\nbind = {{ one-password = \"op://shelf-dev/api-token/credential\" }}\n"
    );
    let checkout = shelf().with("starmap.toml", &starmap);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(declarations.refusals(), &[]);
    let secrets = declarations
        .provides()
        .iter()
        .filter(|provide| provide.kind() == NeedKind::Secret)
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
    let names: Vec<String> = provide_names(&declarations)
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

/// Returns an mxv file with one process and the need `need`, a table body.
fn mxv_with_need(need: &str) -> MapCheckout {
    MapCheckout::default().with(
        "mxv.toml",
        &format!("[processes.web]\nargv = [\"./web\"]\n\n[needs.auth-api]\n{need}\n"),
    )
}

const GOOD_NEED: &str = "repo = \"github.com/acme/auth\"\nfrom = \"api\"\nas = \"AUTH_URL\"";

#[test]
fn a_need_with_every_key_valid_is_read() {
    let declarations = read_one("github.com/acme/shelf", &mxv_with_need(GOOD_NEED));
    assert_eq!(declarations.refusals(), &[]);
    assert_eq!(declarations.needs().len(), 1);
}

#[test]
fn a_need_that_is_not_a_table_is_refused_at_its_line() {
    let checkout = MapCheckout::default().with(
        "mxv.toml",
        "[processes.web]\nargv = [\"./web\"]\n\n[needs]\nauth-api = 3\n",
    );
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::need"]);
    assert_eq!(declarations.refusals()[0].line(), Some(5));
}

#[test]
fn a_need_with_a_key_it_does_not_have_is_refused() {
    let need = format!("{GOOD_NEED}\nkind = \"address\"");
    let declarations = read_one("github.com/acme/shelf", &mxv_with_need(&need));
    assert_eq!(codes(&declarations), ["jumpgate::declaration::need"]);
}

#[test]
fn a_need_without_a_required_key_is_refused_for_each_key() {
    for missing in ["repo", "from", "as"] {
        let need: Vec<&str> = GOOD_NEED
            .lines()
            .filter(|line| !line.starts_with(missing))
            .collect();
        let declarations = read_one("github.com/acme/shelf", &mxv_with_need(&need.join("\n")));
        assert_eq!(
            codes(&declarations),
            ["jumpgate::declaration::need"],
            "{missing}"
        );
    }
}

#[test]
fn a_need_with_a_bad_value_is_refused_for_each_key() {
    for (key, bad) in [
        ("repo", "repo = \"auth\""),
        ("repo", "repo = 3"),
        ("from", "from = \"Api\""),
        ("as", "as = \"1BAD\""),
        ("optional", "optional = \"yes\""),
    ] {
        let need: Vec<String> = GOOD_NEED
            .lines()
            .filter(|line| !line.starts_with(key))
            .map(str::to_owned)
            .chain([bad.to_owned()])
            .collect();
        let declarations = read_one("github.com/acme/shelf", &mxv_with_need(&need.join("\n")));
        assert_eq!(
            codes(&declarations),
            ["jumpgate::declaration::need"],
            "{bad}"
        );
    }
}

#[test]
fn a_need_named_with_an_uppercase_letter_is_refused() {
    let checkout = MapCheckout::default().with(
        "mxv.toml",
        &format!("[processes.web]\nargv = [\"./web\"]\n\n[needs.Auth]\n{GOOD_NEED}\n"),
    );
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::need"]);
}

#[test]
fn a_refused_need_leaves_the_other_needs_and_the_secrets_read() {
    let mxv = format!(
        "[processes.web]\nargv = [\"./web\"]\n\n[needs.bad]\nrepo = 3\n\n[needs.good]\n{GOOD_NEED}\n"
    );
    let checkout = MapCheckout::default()
        .with("mxv.toml", &mxv)
        .with("starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::need"]);
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
fn a_need_in_an_app_with_no_unit_is_refused() {
    let checkout =
        MapCheckout::default().with("mxv.toml", &format!("[needs.auth-api]\n{GOOD_NEED}\n"));
    let declarations = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::consumers"]);
}

#[test]
fn a_need_named_like_a_secret_clashes() {
    let need = GOOD_NEED;
    let checkout = MapCheckout::default()
        .with(
            "mxv.toml",
            &format!("[processes.web]\nargv = [\"./web\"]\n\n[needs.api-token]\n{need}\n"),
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
fn a_starmap_file_that_does_not_parse_is_refused_at_its_line_and_the_needs_still_read() {
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
                .provides()
                .iter()
                .map(|provide| serde_json::to_string(provide).unwrap()),
        )
        .collect();
    assert!(text.contains("op://shelf-dev/api-token/credential"));
    assert!(!text.contains("The token shelf sends"));
}

#[test]
fn a_refusal_never_quotes_a_value_of_the_file() {
    let checkout = MapCheckout::default().with(
        "mxv.toml",
        "[processes.web]\nargv = [\"./web\"]\n\n[needs.auth-api]\nrepo = \"s3cr3t-token\"\nfrom = \"api\"\nas = \"X\"\n",
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
    assert_eq!(Reason::ALL.len(), 15);
}

#[test]
fn the_two_declaration_file_names_are_valid_repository_paths() {
    assert_eq!(RepoFilePath::known(MXV_FILE), MXV_FILE.parse().unwrap());
    assert_eq!(
        RepoFilePath::known(STARMAP_FILE),
        STARMAP_FILE.parse().unwrap()
    );
}

/// A starmap file with one secret whose `unset` is `unset`.
fn secret_checkout(unset: &str) -> MapCheckout {
    let starmap = format!(
        "[variables.K]\npurpose = \"p\"\ntype = \"string\"\nunset = {unset}\nshow = \"withheld\"\nsource = \"secret\"\n"
    );
    MapCheckout::default()
        .with("mxv.toml", "[processes.web]\nargv = [\"./web\"]\n")
        .with("starmap.toml", &starmap)
}

#[test]
fn a_secret_with_a_default_is_refused_because_starmap_has_no_such_secret() {
    let declarations = read_one(
        "github.com/acme/web",
        &secret_checkout("{ default = \"x\" }"),
    );
    assert_eq!(codes(&declarations), ["jumpgate::declaration::starmap"]);
    assert_eq!(declarations.needs(), &[]);
    let required = read_one("github.com/acme/web", &secret_checkout("\"required\""));
    assert_eq!(required.refusals(), &[]);
    assert_eq!(required.needs().len(), 1);
}

#[test]
fn two_repositories_whose_ids_end_in_one_segment_are_refused_as_one_app() {
    let (first, second) = (id("github.com/acme/auth"), id("github.com/evil/auth"));
    let (one, two) = (auth(), auth());
    let declarations = read(&[
        Repository::new(&first, &one),
        Repository::new(&second, &two),
    ]);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::clash"]);
    assert_eq!(declarations.refusals()[0].repo(), &second);
    assert!(
        declarations
            .units()
            .iter()
            .all(|unit| unit.repo() == &first)
    );
    let third = id("github.com/evil/auth2");
    let three = auth();
    let apart = read(&[
        Repository::new(&first, &one),
        Repository::new(&third, &three),
    ]);
    assert_eq!(apart.refusals(), &[]);
}

#[test]
fn a_secret_provide_name_comes_from_its_reference_alone() {
    let bound = |reference: &str| {
        let starmap = format!(
            "[variables.K]\npurpose = \"p\"\ntype = \"string\"\nunset = \"required\"\nshow = \"withheld\"\nsource = \"secret\"\nbind = {{ one-password = \"{reference}\" }}\n"
        );
        MapCheckout::default()
            .with("mxv.toml", "[processes.web]\nargv = [\"./web\"]\n")
            .with("starmap.toml", &starmap)
    };
    let (web, other) = (id("github.com/acme/web"), id("github.com/acme/other"));
    let (alone, together) = (bound("op://a/b/c"), bound("op://a-b/c"));
    let first = read(&[Repository::new(&web, &alone)]);
    let both = read(&[
        Repository::new(&web, &alone),
        Repository::new(&other, &together),
    ]);
    let secret_provide = |declarations: &Declarations| {
        provide_names(declarations)
            .into_iter()
            .find(|path| path.contains("a-b-c"))
            .unwrap()
    };
    assert_eq!(first.refusals(), &[]);
    assert!(provide_names(&both).contains(&secret_provide(&first)));
}

#[test]
fn a_need_of_a_name_that_two_kinds_share_is_refused_and_the_suffixed_name_is_accepted() {
    let bare = mxv_with_need("repo = \"github.com/acme/shelf\"\nfrom = \"db\"\nas = \"DB\"");
    let (shelf_id, web_id) = (id("github.com/acme/shelf"), id("github.com/acme/web"));
    let shelf = shelf();
    let refused = read(&[
        Repository::new(&shelf_id, &shelf),
        Repository::new(&web_id, &bare),
    ]);
    assert_eq!(codes(&refused), ["jumpgate::declaration::provider"]);
    let kinded = mxv_with_need("repo = \"github.com/acme/shelf\"\nfrom = \"db-port\"\nas = \"DB\"");
    let accepted = read(&[
        Repository::new(&shelf_id, &shelf),
        Repository::new(&web_id, &kinded),
    ]);
    assert_eq!(accepted.refusals(), &[]);
    assert_eq!(accepted.needs().len(), 4);
}

const DEADBAND_MXV: &str = include_str!("../../../../fixtures/sockets/deadband/mxv.toml");
const MULTIVERSE_MXV: &str = include_str!("../../../../fixtures/sockets/multiverse/mxv.toml");

const DEADBAND_ROOT: &str = "/work/deadband";

fn deadband() -> MapCheckout {
    MapCheckout::default()
        .with("mxv.toml", DEADBAND_MXV)
        .at(DEADBAND_ROOT)
}

fn multiverse() -> MapCheckout {
    MapCheckout::default().with("mxv.toml", MULTIVERSE_MXV)
}

/// Reads deadband and multiverse, or multiverse alone.
fn read_sockets(with_deadband: bool) -> Declarations {
    let (deadband_id, multiverse_id) = (
        id("github.com/crumbkit/deadband"),
        id("github.com/crumbkit/multiverse"),
    );
    let (deadband, multiverse) = (deadband(), multiverse());
    let mut repositories = vec![Repository::new(&multiverse_id, &multiverse)];
    if with_deadband {
        repositories.push(Repository::new(&deadband_id, &deadband));
    }
    read(&repositories)
}

fn consumer_names(need: &Need) -> Vec<String> {
    need.consumers().iter().map(ToString::to_string).collect()
}

#[test]
fn the_socket_fixtures_are_read_without_a_refusal() {
    let declarations = read_sockets(true);
    assert_eq!(declarations.refusals(), &[]);
}

#[test]
fn a_socket_table_makes_a_provide_of_kind_socket_with_an_absolute_path() {
    let declarations = read_sockets(true);
    let provide = declarations
        .provides()
        .iter()
        .find(|provide| {
            provide.path().to_string() == "/jumpgate/provides/app/deadband/deadband-socket"
        })
        .expect("the socket provide is written");
    assert_eq!(provide.kind(), NeedKind::Socket);
    let Reference::Socket(socket) = provide.reference() else {
        panic!("the reference is a socket");
    };
    assert_eq!(socket.unit().as_str(), "deadband");
    assert_eq!(
        socket.path().as_str(),
        "/work/deadband/target/deadband.sock"
    );
}

#[test]
fn a_need_of_a_socket_provide_takes_the_kind_socket_and_requests_it() {
    let declarations = read_sockets(true);
    let need = need(&declarations, "/jumpgate/needs/multiverse/deadband");
    assert_eq!(need.kind(), NeedKind::Socket);
    let Some(Request::Need(source)) = need.requests() else {
        panic!("the need requests a provide");
    };
    assert_eq!(source.repo().as_str(), "github.com/crumbkit/deadband");
    assert_eq!(source.provide().as_str(), "deadband-socket");
}

#[test]
fn a_need_of_an_app_with_only_tasks_is_received_by_its_commands() {
    let declarations = read_sockets(true);
    assert_eq!(declarations.refusals(), &[]);
    let need = need(&declarations, "/jumpgate/needs/multiverse/deadband");
    assert_eq!(consumer_names(need), ["command build", "command test"]);
    assert_eq!(need.when_unbound(), &WhenUnbound::Omit);
}

#[test]
fn a_need_of_a_provide_in_a_repository_that_is_not_read_keeps_its_commands() {
    let declarations = read_sockets(false);
    assert_eq!(declarations.refusals(), &[]);
    let need = need(&declarations, "/jumpgate/needs/multiverse/deadband");
    assert_eq!(consumer_names(need), ["command build", "command test"]);
}

#[test]
fn an_app_with_units_and_tasks_has_its_units_and_commands_as_consumers() {
    let mxv = format!("{SHELF_MXV}\n[tasks.migrate]\nrun = \"./migrate\"\n");
    let shelf = MapCheckout::default()
        .with("mxv.toml", &mxv)
        .with("starmap.toml", SHELF_STARMAP);
    let (shelf_id, auth_id) = (id("github.com/acme/shelf"), id("github.com/acme/auth"));
    let auth = auth();
    let declarations = read(&[
        Repository::new(&shelf_id, &shelf),
        Repository::new(&auth_id, &auth),
    ]);
    assert_eq!(declarations.refusals(), &[]);
    let need = need(&declarations, "/jumpgate/needs/shelf/auth-api");
    assert_eq!(consumer_names(need), ["web", "db", "command migrate"]);
}

#[test]
fn a_secret_reaches_only_the_units_that_declare_it_even_when_the_app_has_tasks() {
    let mxv = format!("{SHELF_MXV}\n[tasks.migrate]\nrun = \"./migrate\"\n");
    let shelf = MapCheckout::default()
        .with("mxv.toml", &mxv)
        .with("starmap.toml", SHELF_STARMAP);
    let declarations = read_one("github.com/acme/shelf", &shelf);
    let secrets: Vec<&Need> = declarations
        .needs()
        .iter()
        .filter(|need| need.kind() == NeedKind::Secret)
        .collect();
    assert!(!secrets.is_empty());
    for need in secrets {
        assert!(
            need.consumers()
                .iter()
                .all(|consumer| consumer.unit().is_some()),
            "{}",
            need.path()
        );
    }
}

#[test]
fn an_app_with_a_need_and_neither_a_unit_nor_a_task_is_refused_with_the_consumers_code() {
    let mxv = mxv_without_units(
        "[needs.auth-api]\nrepo = \"github.com/acme/auth\"\nfrom = \"api\"\nas = \"AUTH_URL\"\n",
    );
    let declarations = read_one("github.com/acme/shelf", &mxv);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::consumers"]);
}

fn mxv_without_units(text: &str) -> MapCheckout {
    MapCheckout::default().with("mxv.toml", text)
}

#[test]
fn a_task_name_that_breaks_the_name_rule_is_refused() {
    let mxv = mxv_without_units("[tasks.\"a b\"]\nrun = \"x\"\n");
    let declarations = read_one("github.com/acme/shelf", &mxv);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::name"]);
}

#[test]
fn a_wants_table_is_refused_with_a_code_that_names_needs() {
    let mxv = mxv_without_units(
        "[tasks.t]\nrun = \"x\"\n\n[wants.auth-api]\nrepo = \"github.com/acme/auth\"\nprovides = \"api\"\nas = \"AUTH_URL\"\n",
    );
    let declarations = read_one("github.com/acme/shelf", &mxv);
    assert_eq!(codes(&declarations), ["jumpgate::declaration::use_needs"]);
    assert!(declarations.refusals()[0].message().contains("[needs."));
    assert_eq!(declarations.needs().len(), 0);
}

#[test]
fn an_offers_table_is_refused_with_a_code_that_names_provides() {
    let mxv = mxv_without_units("[offers.x]\nkind = \"socket\"\n");
    let declarations = read_one("github.com/acme/shelf", &mxv);
    assert_eq!(
        codes(&declarations),
        ["jumpgate::declaration::use_provides"]
    );
    let message = declarations.refusals()[0].message();
    assert!(
        message.contains("new keys kind, unit and path"),
        "{message}"
    );
    assert!(declarations.refusals()[0].message().contains("[provides."));
}

/// Returns a checkout of root `/work/shelf` whose `mxv.toml` is `text`.
fn rooted(text: &str) -> MapCheckout {
    MapCheckout::default()
        .with("mxv.toml", text)
        .at("/work/shelf")
}

const SOCKET_APP: &str = "[processes.web]\nargv = [\"./web\"]\n\n";

#[test]
fn a_socket_provide_with_every_key_valid_is_read_and_each_refusal_has_a_twin() {
    let good =
        "[provides.web-socket]\nkind = \"socket\"\nunit = \"web\"\npath = \"run/web.sock\"\n";
    let accepted = read_one(
        "github.com/acme/shelf",
        &rooted(&format!("{SOCKET_APP}{good}")),
    );
    assert_eq!(accepted.refusals(), &[]);
    assert_eq!(
        provide_names(&accepted),
        [
            "/jumpgate/provides/app/shelf/web",
            "/jumpgate/provides/app/shelf/web-socket"
        ]
    );
    let cases = [
        (
            "a unit the app lacks",
            good.replace("unit = \"web\"", "unit = \"api\""),
        ),
        (
            "a path outside the checkout",
            good.replace("run/web.sock", "../web.sock"),
        ),
        (
            "an absolute path",
            good.replace("run/web.sock", "/run/web.sock"),
        ),
        ("an unknown kind", good.replace("socket\"", "pipe\"")),
        ("an unknown key", format!("{good}mode = \"0600\"\n")),
        (
            "a missing path",
            good.replace("path = \"run/web.sock\"\n", ""),
        ),
        ("a missing unit", good.replace("unit = \"web\"\n", "")),
        (
            "a name with a capital",
            good.replace("web-socket", "Web-socket"),
        ),
        (
            "a value that is not a table",
            "[provides]\nweb-socket = \"x\"\n".to_owned(),
        ),
    ];
    for (what, table) in cases {
        let refused = read_one(
            "github.com/acme/shelf",
            &rooted(&format!("{SOCKET_APP}{table}")),
        );
        assert_eq!(
            codes(&refused),
            ["jumpgate::declaration::provide"],
            "{what}"
        );
    }
}

#[test]
fn a_socket_provide_in_a_checkout_with_no_directory_is_refused() {
    let good =
        "[provides.web-socket]\nkind = \"socket\"\nunit = \"web\"\npath = \"run/web.sock\"\n";
    let checkout = mxv_without_units(&format!("{SOCKET_APP}{good}"));
    let refused = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&refused), ["jumpgate::declaration::provide"]);
}

#[test]
fn a_checkout_with_no_directory_says_why_in_the_refusal() {
    struct Odd(MapCheckout);
    impl Checkout for Odd {
        fn root(&self) -> Result<&str, CheckoutError> {
            Err(CheckoutError::new(
                "the path of its directory is not UTF-8 text",
            ))
        }
        fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
            self.0.read(path)
        }
    }
    let good =
        "[provides.web-socket]\nkind = \"socket\"\nunit = \"web\"\npath = \"run/web.sock\"\n";
    let checkout = Odd(MapCheckout::default().with("mxv.toml", &format!("{SOCKET_APP}{good}")));
    let refused = read_one("github.com/acme/shelf", &checkout);
    assert_eq!(codes(&refused), ["jumpgate::declaration::provide"]);
    let message = refused.refusals()[0].message();
    assert!(message.contains("not UTF-8 text"), "{message}");
    assert!(!message.contains("not a directory"), "{message}");
}

/// A checkout whose root has `bytes` bytes once the socket `run/web.sock` is
/// joined to it.
fn root_for_socket_of(bytes: usize) -> MapCheckout {
    let good =
        "[provides.web-socket]\nkind = \"socket\"\nunit = \"web\"\npath = \"run/web.sock\"\n";
    let tail = "/run/web.sock".len();
    let root = format!("/{}", "d".repeat(bytes - tail - 1));
    MapCheckout::default()
        .with("mxv.toml", &format!("{SOCKET_APP}{good}"))
        .at(&root)
}

#[test]
fn a_socket_path_longer_than_the_platform_limit_is_refused_and_one_at_the_limit_is_read() {
    let limit = crate::declaration::mxv::MAX_SOCKET_PATH_BYTES;
    let at = read_one("github.com/acme/shelf", &root_for_socket_of(limit));
    assert_eq!(at.refusals(), &[]);
    let over = read_one("github.com/acme/shelf", &root_for_socket_of(limit + 1));
    assert_eq!(codes(&over), ["jumpgate::declaration::socket_path"]);
    assert!(over.refusals()[0].message().contains("at most"));
    assert!(!provide_names(&over).contains(&"/jumpgate/provides/app/shelf/web-socket".to_owned()));
}

#[test]
fn a_socket_provide_may_name_a_container_of_the_app() {
    let text = "[containers.db]\nimage = \"postgres:17\"\n\n[provides.db-socket]\nkind = \"socket\"\nunit = \"db\"\npath = \"run/db.sock\"\n";
    let declarations = read_one("github.com/acme/shelf", &rooted(text));
    assert_eq!(declarations.refusals(), &[]);
    let provide = declarations
        .provides()
        .iter()
        .find(|provide| provide.path().to_string().ends_with("db-socket"))
        .expect("the socket provide is written");
    let Reference::Socket(socket) = provide.reference() else {
        panic!("the reference is a socket");
    };
    assert_eq!(socket.unit().as_str(), "db");
    assert_eq!(socket.path().as_str(), "/work/shelf/run/db.sock");
}

#[test]
fn a_socket_provide_that_takes_the_name_of_a_port_clashes() {
    let text = format!(
        "ports = [\"sock\"]\n\n{SOCKET_APP}[provides.sock]\nkind = \"socket\"\nunit = \"web\"\npath = \"run/web.sock\"\n"
    );
    let refused = read_one("github.com/acme/shelf", &rooted(&text));
    assert_eq!(codes(&refused), ["jumpgate::declaration::clash"]);
}

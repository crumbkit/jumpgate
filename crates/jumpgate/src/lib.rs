//! jumpgate matches what each app needs to what a provider offers, for one
//! target, and writes the result as a plan that holds no secret value.
//!
//! This crate holds the five record types and their validation:
//!
//! - [`Need`], `jumpgate/need/v1`: one thing an app requires.
//! - [`Offer`], `jumpgate/offer/v1`: something a provider can supply.
//! - [`Binding`], `jumpgate/binding/v1`: a need matched to an offer for one
//!   target.
//! - [`Target`], `jumpgate/target/v1`: a target and the board whose bindings
//!   count for it.
//! - [`Unit`], `jumpgate/unit/v1`: a process or container that an app runs.
//!
//! [`matcher::resolve`] matches the needs of a store to its offers for one
//! target with a [`matcher::Matcher`], and returns the [`plan::Plan`], which
//! is `jumpgate/plan/v1alpha1`.
//!
//! Every string a record holds has a type of its own, such as [`AppName`],
//! [`RepoId`] or [`SecretReference`], whose [`FromStr`](std::str::FromStr)
//! is its only constructor. An offer and a binding hold references and have
//! no field that can hold a secret value. A refused value or record returns
//! a [`RecordError`] whose [`Code`] is `jumpgate::record::<reason>`, and
//! whose message never quotes the refused text.
//!
//! The crate runs no process and reads no secret store or environment
//! variable. Its only file access is the directory store, which reads and
//! writes and removes the files of one directory that the caller names, and
//! writes a plan file where the caller says.
//!
//! ```
//! use jumpgate::{Need, Record};
//!
//! let text = r#"{
//!     "$type": "jumpgate/need/v1",
//!     "app": "shelf",
//!     "name": "api_token",
//!     "kind": "secret",
//!     "consumers": ["api"],
//!     "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
//!     "delivers_as": {"variables": ["API_TOKEN"]},
//!     "optional": false,
//!     "when": [],
//!     "when_unbound": "fail"
//! }"#;
//! let need = Need::from_json(text)?;
//! assert_eq!(need.path().to_string(), "/jumpgate/needs/shelf/api_token");
//! # Ok::<(), jumpgate::RecordError>(())
//! ```

#[macro_use]
mod text;

pub mod check;
pub mod declaration;
mod distinct;
mod error;
mod fallback;
mod file_path;
mod json;
mod keyword;
pub mod matcher;
mod name;
mod path;
pub mod plan;
pub mod record;
mod repo;
mod secret;
pub mod store;
mod variable;

pub use crate::distinct::Distinct;
pub use crate::error::{Code, Reason, RecordError};
pub use crate::fallback::{FallbackValue, MAX_FALLBACK_BYTES};
pub use crate::file_path::{DeliveryPath, MAX_FILE_PATH_BYTES, RepoFilePath};
pub use crate::keyword::{NeedKind, ProviderKind, Scope, ServiceKind, TargetKind, UnitKind};
pub use crate::name::{
    AppName, Audience, BoardId, MAX_NAME_BYTES, NeedName, OfferName, PortName, ProfileName,
    Projection, ProviderName, ServiceName, TargetName, UnitName,
};
pub use crate::path::{BindingPath, NeedPath, OfferPath, RecordPath, TargetPath, UnitPath};
pub use crate::record::{
    Binding, Condition, DeclaredIn, DeliversAs, KindMismatch, Need, Offer, PortReference, Provider,
    Record, RecordType, Reference, RouteReference, ServiceReference, Target, Unit, UnitReference,
    WhenUnbound, pair_service,
};
pub use crate::repo::{MAX_REPO_ID_BYTES, RepoId};
pub use crate::secret::SecretReference;
pub use crate::variable::{MAX_VARIABLE_BYTES, VariableName};

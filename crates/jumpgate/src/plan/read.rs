//! Reading a plan from its JSON text.
//!
//! The reader walks the JSON value field by field, as the record reader
//! does, so a refusal carries a code of the form `jumpgate::record::<reason>`
//! and the field where it was found, and never quotes the refused text. A
//! plan that another matcher or a person wrote reaches the checks only
//! after this reader accepts it.

use serde_json::Value;

use crate::error::{Reason, RecordError};
use crate::json::{FromJson, Object, one_key, shape, text};
use crate::path::OfferPath;
use crate::plan::{
    Adapter, MAX_PLAN_BYTES, PLAN_TYPE, Plan, PlanBinding, PlanTarget, PlanTypeTag, PlanUnit,
    Supplies, Unbound, UnboundAction, UnboundReason, Via,
};
use crate::record::{Reference, parse_typed};

impl Plan {
    /// Reads a plan from its JSON text, whichever matcher wrote it.
    ///
    /// # Errors
    ///
    /// Returns a [`RecordError`] when the text is longer than
    /// [`MAX_PLAN_BYTES`], is not JSON, names another `$type`, lacks a
    /// field, holds a field the plan does not have, or holds a value that
    /// breaks its rule. The `adapter` block must be empty, because only a
    /// target adapter writes into it and no adapter is built yet. The
    /// error's message never quotes the refused text.
    pub fn from_json(text: &str) -> Result<Self, RecordError> {
        let value = parse_typed(text, MAX_PLAN_BYTES, PLAN_TYPE)?;
        let object = Object::open(&value, &["$type", "target", "unbound", "units", "adapter"])?;
        let adapter: Adapter = object.required("adapter")?;
        Ok(Self {
            plan_type: PlanTypeTag,
            target: object.required("target")?,
            unbound: object.required("unbound")?,
            units: object.required("units")?,
            adapter,
        })
    }
}

impl FromJson for Adapter {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        Object::open(value, &[])?;
        Ok(Self {})
    }
}

impl FromJson for PlanTarget {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(value, &["name", "kind"])?;
        Ok(Self {
            name: object.required("name")?,
            kind: object.required("kind")?,
        })
    }
}

impl FromJson for PlanUnit {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(
            value,
            &[
                "app",
                "name",
                "repo",
                "kind",
                "starts_after",
                "addresses",
                "bindings",
            ],
        )?;
        Ok(Self {
            app: object.required("app")?,
            name: object.required("name")?,
            repo: object.required("repo")?,
            kind: object.required("kind")?,
            starts_after: object.required("starts_after")?,
            addresses: object.required("addresses")?,
            bindings: object.required("bindings")?,
        })
    }
}

impl FromJson for PlanBinding {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(
            value,
            &[
                "need",
                "kind",
                "consumers",
                "offer",
                "via",
                "delivers_as",
                "supplies",
                "projection",
                "audience",
            ],
        )?;
        Ok(Self {
            need: object.required("need")?,
            kind: object.required("kind")?,
            consumers: object.required("consumers")?,
            offer: object.optional("offer")?,
            via: object.required("via")?,
            delivers_as: object.required("delivers_as")?,
            supplies: object.required("supplies")?,
            projection: object.optional("projection")?,
            audience: object.optional("audience")?,
        })
    }
}

impl FromJson for Via {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        match text(value)? {
            "binding" => Ok(Self::Binding),
            "want" => Ok(Self::Want),
            "bind" => Ok(Self::Bind),
            "fallback" => Ok(Self::Fallback),
            _ => Err(RecordError::new(
                Reason::Keyword,
                "the value is not one of binding, want, bind and fallback",
            )),
        }
    }
}

impl FromJson for Supplies {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let (key, inner) = one_key(
            value,
            &["reference", "fallback"],
            "the value is not an object with exactly one of the keys reference and fallback",
        )?;
        let supplies = if key == "reference" {
            Reference::from_json(inner).map(Self::Reference)
        } else {
            FromJson::from_json(inner).map(Self::Fallback)
        };
        supplies.map_err(|error| error.within(key))
    }
}

impl FromJson for UnboundAction {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        match text(value)? {
            "fail" => Ok(Self::Fail),
            "omit" => Ok(Self::Omit),
            "fallback" => Ok(Self::Fallback),
            _ => Err(RecordError::new(
                Reason::Keyword,
                "the value is not one of fail, omit and fallback",
            )),
        }
    }
}

impl FromJson for UnboundReason {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        const MESSAGE: &str = "the value is no_offer, fallback_on_cloud, local_only, or an object whose one key is offer_missing, ambiguous or kind_mismatch";
        match value {
            Value::String(word) => match word.as_str() {
                "no_offer" => Ok(Self::NoOffer),
                "fallback_on_cloud" => Ok(Self::FallbackOnCloud),
                "local_only" => Ok(Self::LocalOnly),
                _ => Err(RecordError::new(Reason::Keyword, MESSAGE)),
            },
            Value::Object(_) => {
                let (key, inner) = one_key(
                    value,
                    &["offer_missing", "ambiguous", "kind_mismatch"],
                    MESSAGE,
                )?;
                let reason = if key == "offer_missing" {
                    OfferPath::from_json(inner).map(Self::OfferMissing)
                } else if key == "kind_mismatch" {
                    OfferPath::from_json(inner).map(Self::KindMismatch)
                } else {
                    Vec::<OfferPath>::from_json(inner).map(Self::Ambiguous)
                };
                reason.map_err(|error| error.within(key))
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => {
                Err(shape(MESSAGE))
            }
        }
    }
}

impl FromJson for Unbound {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(
            value,
            &["need", "kind", "consumers", "when_unbound", "reason"],
        )?;
        Ok(Self {
            need: object.required("need")?,
            kind: object.required("kind")?,
            consumers: object.required("consumers")?,
            when_unbound: object.required("when_unbound")?,
            reason: object.required("reason")?,
        })
    }
}

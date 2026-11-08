/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use askama::Template;
use std::fmt::Display;

use crate::backends::{
    CodeDeclaration, CodeOracle, CodeType, LiteralRenderer, TypeIdentifier, VariablesType,
};
use crate::intermediate_representation::{CoproductDef, FeatureManifest, Literal};

use super::filters;

use super::common::{self, code_type};

pub struct CoproductCodeType {
    id: String,
}

impl CoproductCodeType {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl CodeType for CoproductCodeType {
    fn type_label(&self, _oracle: &dyn CodeOracle) -> String {
        common::class_name(&self.id)
    }

    fn property_getter(
        &self,
        oracle: &dyn CodeOracle,
        vars: &dyn Display,
        prop: &dyn Display,
        default: &dyn Display,
    ) -> String {
        code_type::property_getter(self, oracle, vars, prop, default)
    }

    fn value_getter(
        &self,
        oracle: &dyn CodeOracle,
        vars: &dyn Display,
        prop: &dyn Display,
    ) -> String {
        code_type::value_getter(self, oracle, vars, prop)
    }

    fn value_mapper(&self, oracle: &dyn CodeOracle) -> Option<String> {
        code_type::value_mapper(self, oracle)
    }

    fn variables_type(&self, _oracle: &dyn CodeOracle) -> VariablesType {
        VariablesType::Variables
    }

    fn create_transform(&self, oracle: &dyn CodeOracle) -> Option<String> {
        Some(format!("{}.create", self.type_label(oracle)))
    }

    fn merge_transform(&self, oracle: &dyn CodeOracle) -> Option<String> {
        Some(format!("{}.mergeWith", self.type_label(oracle)))
    }

    /// `create` returns `nil` for an unknown variant, so the merger is applied to an optional.
    fn value_merger(&self, _oracle: &dyn CodeOracle, default: &dyn Display) -> Option<String> {
        Some(format!("map {{ $0._mergeWith({}) }}", default))
    }

    fn literal(
        &self,
        oracle: &dyn CodeOracle,
        ctx: &dyn Display,
        renderer: &dyn LiteralRenderer,
        literal: &Literal,
    ) -> String {
        renderer.literal(
            oracle,
            &TypeIdentifier::Coproduct(self.id.clone()),
            literal,
            ctx,
        )
    }
}

/// A variant of a coproduct, as the template needs it.
pub(crate) struct CoproductVariant {
    /// The tag in the JSON, which is the name of the variant's object.
    pub(crate) tag: String,
    /// The name of the enum case.
    pub(crate) case_name: String,
    /// The name of the variant's object class.
    pub(crate) object_class_name: String,
    pub(crate) doc: String,
}

#[derive(Template)]
#[template(syntax = "swift", escape = "none", path = "CoproductTemplate.swift")]
pub(crate) struct CoproductCodeDeclaration {
    inner: CoproductDef,
    fm: FeatureManifest,
}

impl CoproductCodeDeclaration {
    pub fn new(fm: &FeatureManifest, inner: &CoproductDef) -> Self {
        Self {
            fm: fm.clone(),
            inner: inner.clone(),
        }
    }

    pub fn inner(&self) -> CoproductDef {
        self.inner.clone()
    }

    pub fn variants(&self) -> Vec<CoproductVariant> {
        self.inner
            .variants()
            .iter()
            .map(|tag| CoproductVariant {
                tag: tag.clone(),
                case_name: common::enum_variant_name(tag),
                object_class_name: common::class_name(tag),
                doc: self
                    .fm
                    .find_object(tag)
                    .map(|o| o.doc())
                    .unwrap_or_default(),
            })
            .collect()
    }
}

impl CodeDeclaration for CoproductCodeDeclaration {
    fn definition_code(&self, _oracle: &dyn CodeOracle) -> Option<String> {
        Some(self.render().unwrap())
    }
}

/// Renders a literal of a coproduct, for example `Promo.banner(Banner(title: "Hi"))`.
///
/// `object_literal` renders the variant's object, from the literal without the discriminant.
pub(crate) fn coproduct_literal(
    fm: &FeatureManifest,
    renderer: &dyn LiteralRenderer,
    oracle: &dyn CodeOracle,
    id: &str,
    value: &Literal,
    ctx: &dyn Display,
) -> String {
    let def = fm.find_coproduct(id).unwrap();
    let Literal::Object(map) = value else {
        unreachable!("A JSON object is expected for the {id} coproduct literal")
    };
    let tag = def
        .tag(map)
        .unwrap_or_else(|| unreachable!("The {id} coproduct literal has an invalid tag"));

    let mut map = map.clone();
    map.remove(def.discriminant());
    let variant = TypeIdentifier::Object(tag.to_string());
    let object =
        super::object::object_literal(fm, renderer, oracle, &variant, &Literal::Object(map), ctx);

    format!(
        "{coproduct}.{case}({object})",
        coproduct = common::class_name(&id),
        case = common::enum_variant_name(&tag),
    )
}

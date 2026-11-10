// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::{BTreeMap, HashSet};

use crate::intermediate_representation::{
    CoproductDef, FeatureDef, ObjectDef, TypeFinder, TypeRef,
};

pub(crate) struct TypeQuery<'a> {
    object_defs: &'a BTreeMap<String, ObjectDef>,
    // Used when coproducts are added to the types.
    #[allow(dead_code)]
    coproduct_defs: &'a BTreeMap<String, CoproductDef>,
}

impl<'a> TypeQuery<'a> {
    pub(crate) fn new(
        objs: &'a BTreeMap<String, ObjectDef>,
        coproducts: &'a BTreeMap<String, CoproductDef>,
    ) -> Self {
        Self {
            object_defs: objs,
            coproduct_defs: coproducts,
        }
    }

    pub(crate) fn all_types(&self, feature_def: &FeatureDef) -> HashSet<TypeRef> {
        let mut types = Default::default();
        self.gather_types(&feature_def.all_types(), &mut types);
        types
    }

    fn gather_types(&self, unseen: &HashSet<TypeRef>, seen: &mut HashSet<TypeRef>) {
        for t in unseen {
            if !seen.contains(t) {
                seen.insert(t.clone());
                if let TypeRef::Object(nm) = t {
                    let def = self.object_defs.get(nm).unwrap();
                    self.gather_types(&def.all_types(), seen);
                }
            }
        }
    }
}

// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use moor_common::{
    model::{ObjAttrs, ObjectKind, VerbArgsSpec, VerbFlag},
    util::BitEnum,
};
use moor_compiler::CompileOptions;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_var::{
    NOTHING, Obj, SYSTEM_OBJECT, Symbol,
    program::ProgramType,
    v_int, v_str,
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::sync::Arc;

/// Configuration for deterministic test database generation.
#[derive(Clone, Debug)]
pub struct DbBuildConfig {
    pub object_count: usize,
    pub inheritance_stride: usize,
    pub props_per_object: usize,
    pub verbs_per_object: usize,
    pub prop_override_ratio: f32,
    pub verb_override_ratio: f32,
    pub rng_seed: u64,
}

pub struct GeneratedDb {
    pub db: Arc<TxDB>,
    pub object_ids: Vec<Obj>,
}

/// Builder for generating deterministic inheritance graphs with optional overrides.
pub struct TestDbBuilder {
    config: DbBuildConfig,
}

impl TestDbBuilder {
    pub fn new(config: DbBuildConfig) -> Self {
        Self { config }
    }

    pub fn build(&self) -> GeneratedDb {
        let object_count = self.config.object_count.max(1);
        let inheritance_stride = self.config.inheritance_stride.max(1);
        let mut rng = StdRng::seed_from_u64(self.config.rng_seed);
        let empty_program = ProgramType::MooR(
            moor_compiler::compile("", CompileOptions::default()).expect("compile empty program"),
        );
        let non_empty_program = ProgramType::MooR(
            moor_compiler::compile("return 1;", CompileOptions::default())
                .expect("compile test program"),
        );
        let verb_0_0 = Symbol::mk("verb_0_0");

        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);
        let mut loader = db.loader_client().expect("loader client");

        let mut object_ids = Vec::with_capacity(object_count);
        let mut all_props_by_obj: Vec<Vec<Symbol>> = Vec::with_capacity(object_count);
        let mut all_verbs_by_obj: Vec<Vec<Symbol>> = Vec::with_capacity(object_count);

        for index in 0..object_count {
            let (parent, name, objkind) = if index == 0 {
                (NOTHING, "system".to_string(), ObjectKind::Objid(SYSTEM_OBJECT))
            } else {
                let parent_index = (index - 1) / inheritance_stride;
                (object_ids[parent_index], format!("obj_{index}"), ObjectKind::NextObjid)
            };

            let obj = loader
                .create_object(
                    objkind,
                    &ObjAttrs::new(SYSTEM_OBJECT, parent, NOTHING, BitEnum::new(), &name),
                )
                .expect("create object");
            object_ids.push(obj);

            let mut local_props = Vec::with_capacity(self.config.props_per_object);
            for prop_index in 0..self.config.props_per_object {
                let prop_name = Symbol::mk(&format!("prop_{index}_{prop_index}"));
                let value = if prop_index % 2 == 0 {
                    v_int((index as i64) * 1000 + prop_index as i64)
                } else {
                    v_str(&format!("value_{index}_{prop_index}"))
                };
                loader
                    .define_property(
                        &obj,
                        &obj,
                        prop_name,
                        &SYSTEM_OBJECT,
                        BitEnum::new(),
                        Some(value),
                    )
                    .expect("define property");
                local_props.push(prop_name);
            }

            let mut local_verbs = Vec::with_capacity(self.config.verbs_per_object);
            for verb_index in 0..self.config.verbs_per_object {
                let verb_name = Symbol::mk(&format!("verb_{index}_{verb_index}"));
                let program = if verb_index == 0 {
                    non_empty_program.clone()
                } else {
                    empty_program.clone()
                };
                loader
                    .add_verb(
                        &obj,
                        &[verb_name],
                        &SYSTEM_OBJECT,
                        BitEnum::new_with(VerbFlag::Exec),
                        VerbArgsSpec::this_none_this(),
                        program,
                    )
                    .expect("define verb");
                local_verbs.push(verb_name);
            }

            let mut all_props = local_props.clone();
            let mut all_verbs = local_verbs.clone();
            if index > 0 {
                let inherited_props = &all_props_by_obj[(index - 1) / inheritance_stride];
                let inherited_verbs = &all_verbs_by_obj[(index - 1) / inheritance_stride];
                all_props.extend_from_slice(inherited_props);
                all_verbs.extend_from_slice(inherited_verbs);

                for prop_name in inherited_props {
                    if rng.random_range(0.0..1.0) < self.config.prop_override_ratio {
                        let value = v_str(&format!("override_{index}_{}", prop_name));
                        loader
                            .set_property(
                                &obj,
                                *prop_name,
                                Some(SYSTEM_OBJECT),
                                Some(BitEnum::new()),
                                Some(value),
                            )
                            .expect("override property");
                    }
                }

                for verb_name in inherited_verbs {
                    if rng.random_range(0.0..1.0) < self.config.verb_override_ratio {
                        let program = if *verb_name == verb_0_0 {
                            non_empty_program.clone()
                        } else {
                            empty_program.clone()
                        };
                        loader
                            .add_verb(
                                &obj,
                                &[*verb_name],
                                &SYSTEM_OBJECT,
                                BitEnum::new_with(VerbFlag::Exec),
                                VerbArgsSpec::this_none_this(),
                                program,
                            )
                            .expect("override verb");
                    }
                }
            }

            all_props_by_obj.push(all_props);
            all_verbs_by_obj.push(all_verbs);
        }

        loader.commit().expect("commit");

        GeneratedDb { db, object_ids }
    }
}

impl Default for DbBuildConfig {
    fn default() -> Self {
        Self {
            object_count: 500,
            inheritance_stride: 3,
            props_per_object: 5,
            verbs_per_object: 3,
            prop_override_ratio: 0.2,
            verb_override_ratio: 0.2,
            rng_seed: 42,
        }
    }
}

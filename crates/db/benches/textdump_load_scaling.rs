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

#![recursion_limit = "256"]

#[path = "../tests/support/mod.rs"]
mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use moor_common::model::{
    CommitResult, ObjAttrs, PropDefs, PropPerms, VerbDef, VerbDefs, WorldStateError,
    loader::{LoaderInterface, ProgressEvent, ProgressPhase},
};
use moor_compiler::CompileOptions;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_textdump::{TextdumpImportOptions, read_textdump};
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use uuid::Uuid;
use semver::Version;
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static MEASURED_MEANS: OnceLock<Mutex<HashMap<usize, f64>>> = OnceLock::new();

fn textdump_load_scaling(c: &mut Criterion) {
    let sizes = [50_000, 100_000, 200_000];
    let dumps = build_dumps(&sizes);

    let mut group = c.benchmark_group("textdump_load_scaling");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    for (size, dump) in &dumps {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), dump, |b, dump| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    let db = load_dump(dump);
                    total += start.elapsed();

                    // Wait for the commit thread to settle before dropping the DB.
                    let _ = db.create_snapshot();
                }
                record_mean(*size, total, iters);
                total
            });
        });
    }
    group.finish();

    emit_scaling_summary(&sizes);
}

fn build_dumps(sizes: &[usize]) -> Vec<(usize, Vec<u8>)> {
    sizes
        .iter()
        .map(|size| {
            let mut config = support::builder::DbBuildConfig::default();
            config.object_count = *size;
            let generated = support::builder::TestDbBuilder::new(config).build();
            let snapshot = generated.db.create_snapshot().expect("snapshot");
            let mut dump = Vec::new();
            support::textdump_writer::write_textdump(
                snapshot.as_ref(),
                &mut dump,
                &support::textdump_writer::TextdumpWriteConfig::default(),
            )
            .expect("write textdump");
            (*size, dump)
        })
        .collect()
}

fn load_dump(dump: &[u8]) -> TxDB {
    let (db, _) = TxDB::open(None, DatabaseConfig::default());
    let loader = db.loader_client().expect("loader client");
    let mut loader = ProgressingLoader::new(loader);
    let cursor = BufReader::new(Cursor::new(dump));
    read_textdump(
        &mut loader,
        cursor,
        Version::new(0, 1, 0),
        CompileOptions::default(),
        TextdumpImportOptions::default(),
    )
    .expect("read textdump");
    let commit = Box::new(loader).commit().expect("commit");
    assert!(matches!(commit, CommitResult::Success { .. }));
    db
}

struct ProgressingLoader {
    inner: Box<dyn LoaderInterface>,
    last_phase_percent: u8,
    last_overall_percent: u8,
}

impl ProgressingLoader {
    fn new(inner: Box<dyn LoaderInterface>) -> Self {
        Self {
            inner,
            last_phase_percent: 0,
            last_overall_percent: 0,
        }
    }

    fn percent(done: u64, total: u64) -> u8 {
        if total == 0 {
            100
        } else {
            ((done * 100) / total) as u8
        }
    }
}

impl LoaderInterface for ProgressingLoader {
    fn create_object(
        &mut self,
        objid: moor_common::model::ObjectKind,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError> {
        self.inner.create_object(objid, attrs)
    }

    fn set_object_parent(
        &mut self,
        obj: &Obj,
        parent: &Obj,
        validate: bool,
    ) -> Result<(), WorldStateError> {
        self.inner.set_object_parent(obj, parent, validate)
    }

    fn set_object_location(&mut self, o: &Obj, location: &Obj) -> Result<(), WorldStateError> {
        self.inner.set_object_location(o, location)
    }

    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError> {
        self.inner.set_object_owner(obj, owner)
    }

    fn set_object_name(&mut self, obj: &Obj, name: String) -> Result<(), WorldStateError> {
        self.inner.set_object_name(obj, name)
    }

    fn add_verb(
        &mut self,
        obj: &Obj,
        names: &[Symbol],
        owner: &Obj,
        flags: moor_common::util::BitEnum<moor_common::model::VerbFlag>,
        args: moor_common::model::VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError> {
        self.inner.add_verb(obj, names, owner, flags, args, program)
    }

    fn update_verb(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        names: &[Symbol],
        owner: &Obj,
        flags: moor_common::util::BitEnum<moor_common::model::VerbFlag>,
        args: moor_common::model::VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError> {
        self.inner
            .update_verb(obj, uuid, names, owner, flags, args, program)
    }

    fn define_property(
        &mut self,
        definer: &Obj,
        objid: &Obj,
        propname: Symbol,
        owner: &Obj,
        flags: moor_common::util::BitEnum<moor_common::model::PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        self.inner
            .define_property(definer, objid, propname, owner, flags, value)
    }

    fn set_property(
        &mut self,
        objid: &Obj,
        propname: Symbol,
        owner: Option<Obj>,
        flags: Option<moor_common::util::BitEnum<moor_common::model::PropFlag>>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        self.inner
            .set_property(objid, propname, owner, flags, value)
    }

    fn max_object(&self) -> Result<Obj, WorldStateError> {
        self.inner.max_object()
    }

    fn recycle_object(&mut self, obj: &Obj) -> Result<(), WorldStateError> {
        self.inner.recycle_object(obj)
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError> {
        let ProgressingLoader { inner, .. } = *self;
        inner.commit()
    }

    fn object_exists(&self, objid: &Obj) -> Result<bool, WorldStateError> {
        self.inner.object_exists(objid)
    }

    fn get_existing_object(&self, objid: &Obj) -> Result<Option<ObjAttrs>, WorldStateError> {
        self.inner.get_existing_object(objid)
    }

    fn get_existing_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError> {
        self.inner.get_existing_verbs(objid)
    }

    fn get_existing_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError> {
        self.inner.get_existing_properties(objid)
    }

    fn get_existing_property_value(
        &self,
        obj: &Obj,
        propname: Symbol,
    ) -> Result<Option<(Var, PropPerms)>, WorldStateError> {
        self.inner.get_existing_property_value(obj, propname)
    }

    fn get_existing_verb_by_names(
        &self,
        obj: &Obj,
        names: &[Symbol],
    ) -> Result<Option<(Uuid, VerbDef)>, WorldStateError> {
        self.inner.get_existing_verb_by_names(obj, names)
    }

    fn get_verb_program(&self, obj: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError> {
        self.inner.get_verb_program(obj, uuid)
    }

    fn update_object_flags(
        &mut self,
        obj: &Obj,
        flags: moor_common::util::BitEnum<moor_common::model::ObjFlag>,
    ) -> Result<(), WorldStateError> {
        self.inner.update_object_flags(obj, flags)
    }

    fn delete_property(&mut self, obj: &Obj, propname: Symbol) -> Result<(), WorldStateError> {
        self.inner.delete_property(obj, propname)
    }

    fn remove_verb(&mut self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        self.inner.remove_verb(obj, uuid)
    }

    fn as_world_state(
        self: Box<Self>,
    ) -> Result<Box<dyn moor_common::model::WorldState>, WorldStateError> {
        let ProgressingLoader { inner, .. } = *self;
        inner.as_world_state()
    }

    fn report_progress(&mut self, event: ProgressEvent) {
        let phase_percent = Self::percent(event.phase_done, event.phase_total);
        let overall_percent = Self::percent(event.overall_done, event.overall_total);
        if phase_percent == self.last_phase_percent && overall_percent == self.last_overall_percent
        {
            return;
        }
        self.last_phase_percent = phase_percent;
        self.last_overall_percent = overall_percent;

        let phase = match event.phase {
            ProgressPhase::ReadTextdump => "read",
            ProgressPhase::CreateObjects => "create_objects",
            ProgressPhase::SetAttributes => "set_attributes",
            ProgressPhase::DefineProperties => "define_props",
            ProgressPhase::SetProperties => "set_props",
            ProgressPhase::DefineVerbs => "define_verbs",
            ProgressPhase::CreateSysrefs => "sysrefs",
        };

        eprintln!("progress {phase} {phase_percent}% overall {overall_percent}%");
    }
}

fn record_mean(size: usize, elapsed: Duration, iters: u64) {
    if iters == 0 {
        return;
    }
    let mean = elapsed.as_secs_f64() / iters as f64;
    let map = MEASURED_MEANS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = map.lock() {
        map.insert(size, mean);
    }
}

fn emit_scaling_summary(sizes: &[usize]) {
    let map = MEASURED_MEANS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(map) = map.lock() else {
        return;
    };
    if sizes.iter().any(|size| !map.contains_key(size)) {
        return;
    }

    eprintln!("textdump load scaling summary:");
    for size in sizes {
        if let Some(mean) = map.get(size) {
            eprintln!("  objects={size} mean={mean:.6}s");
        }
    }

    let xs: Vec<f64> = sizes.iter().map(|size| (*size as f64).ln()).collect();
    let ys: Vec<f64> = sizes
        .iter()
        .map(|size| map.get(size).copied().unwrap_or_default().ln())
        .collect();
    let mean_x = xs.iter().sum::<f64>() / xs.len() as f64;
    let mean_y = ys.iter().sum::<f64>() / ys.len() as f64;
    let mut cov = 0.0;
    let mut var = 0.0;
    for (x, y) in xs.iter().zip(ys.iter()) {
        let dx = x - mean_x;
        cov += dx * (y - mean_y);
        var += dx * dx;
    }
    if var == 0.0 {
        return;
    }
    let slope = cov / var;
    eprintln!("  log-log slope ~= {slope:.2}");
}

criterion_group!(benches, textdump_load_scaling);
criterion_main!(benches);

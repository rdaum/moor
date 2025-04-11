use ahash::AHasher;
use moor_common::model::PropDef;
use moor_var::{Obj, Symbol};
use std::hash::BuildHasherDefault;
use std::sync::RwLock;

pub(crate) struct PropResolutionCache {
    inner: RwLock<Inner>,
}

impl PropResolutionCache {
    pub(crate) fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                version: 0,
                orig_version: 0,
                flushed: false,
                entries: im::HashMap::default(),
            }),
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    #[allow(clippy::type_complexity)]
    entries: im::HashMap<(Obj, Symbol), Option<Vec<PropDef>>, BuildHasherDefault<AHasher>>,
}

impl PropResolutionCache {
    pub(crate) fn fork(&self) -> Self {
        let inner = self.inner.read().unwrap();
        let mut forked_inner = inner.clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Self {
            inner: RwLock::new(forked_inner),
        }
    }

    pub(crate) fn has_changed(&self) -> bool {
        let inner = self.inner.read().unwrap();
        inner.version > inner.orig_version
    }

    pub(crate) fn lookup(&self, obj: &Obj, prop: &Symbol) -> Option<Option<Vec<PropDef>>> {
        let inner = self.inner.read().unwrap();
        inner.entries.get(&(obj.clone(), *prop)).cloned()
    }

    pub(crate) fn flush(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.flushed = true;
        inner.version += 1;
        inner.entries.clear();
    }

    pub(crate) fn fill_hit(&self, obj: &Obj, prop: &Symbol, propd: &PropDef) {
        let mut inner = self.inner.write().unwrap();
        inner.version += 1;
        inner.entries.entry((obj.clone(), *prop)).and_modify(|x| {
            if let Some(x) = x {
                x.push(propd.clone());
            } else {
                *x = Some(vec![propd.clone()]);
            }
        });
    }

    pub(crate) fn fill_miss(&self, obj: &Obj, prop: &Symbol) {
        let mut inner = self.inner.write().unwrap();
        inner.version += 1;
        inner.entries.insert((obj.clone(), *prop), None);
    }
}

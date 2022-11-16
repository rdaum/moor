use crate::model::objects::{ObjAttr, ObjAttrs, Objects};
use crate::model::props::{Pid, PropAttr, PropAttrs, PropDefs, PropFlag, Propdef, Properties};
use crate::model::var::{Objid, Var};

use anyhow::Error;
use bincode::config;
use bincode::config::Configuration;
use enumset::EnumSet;
use rusqlite::{Row, Transaction};
use sea_query::QueryStatement::Insert;
use sea_query::{
    BlobSize, ColumnDef, DynIden, Expr, ForeignKey, ForeignKeyAction, Func, Iden, Index, IndexType,
    IntoIden, OnConflict, Query, SqliteQueryBuilder, Table,
};
use sea_query_rusqlite::RusqliteBinder;

#[derive(Iden)]
enum Object {
    Table,
    Oid,
    Owner,
    Location,
    Parent,
    Name,
    Flags,
}

#[derive(Iden)]
enum PropertyDefinition {
    Table,
    Pid,
    Definer,
    Name,
}

#[derive(Iden)]
enum Property {
    Table,
    Pid,
    Value,
    Owner,
    Flags,
}

pub struct SQLiteTx<'a> {
    tx: Transaction<'a>,
    bincode_cfg: Configuration,
}

fn object_attr_to_column<'a>(attr: ObjAttr) -> DynIden {
    match attr {
        ObjAttr::Owner => Object::Owner.into_iden(),
        ObjAttr::Name => Object::Name.into_iden(),
        ObjAttr::Parent => Object::Parent.into_iden(),
        ObjAttr::Location => Object::Location.into_iden(),
        ObjAttr::Flags => Object::Flags.into_iden(),
    }
}

// fn prop_attr_to_column<'a>(attr: PropAttr) -> DynIden {
//     match attr {
//         PropAttr::Value => Property::Value.into_iden(),
//         PropAttr::Owner => PropAttr::Owner.into_iden(),
//         PropAttr::Flags => PropAttr::Flags.into_iden(),
//     }
// }

fn retr_objid(r: &Row, c_num: usize) -> Result<Option<Objid>, rusqlite::Error> {
    let x: Option<i64> = r.get(c_num)?;
    Ok(x.map(Objid))
}

impl<'a> SQLiteTx<'a> {
    pub fn new(tx: Transaction<'a>) -> Result<Self, anyhow::Error> {
        let s = Self {
            tx,
            bincode_cfg: config::standard(),
        };
        Ok(s)
    }

    pub fn initialize_schema(&self) -> Result<(), anyhow::Error> {
        let object_table_create = Table::create()
            .table(Object::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(Object::Oid)
                    .integer()
                    .primary_key()
                    .not_null(),
            )
            .col(ColumnDef::new(Object::Owner).integer())
            .col(ColumnDef::new(Object::Location).integer())
            .col(ColumnDef::new(Object::Name).string().not_null())
            .col(ColumnDef::new(Object::Parent).integer())
            .col(ColumnDef::new(Object::Flags).integer().not_null())
            .build(SqliteQueryBuilder);

        let property_def_table_create = Table::create()
            .table(PropertyDefinition::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(PropertyDefinition::Pid)
                    .integer()
                    .primary_key()
                    .auto_increment(),
            )
            .col(ColumnDef::new(PropertyDefinition::Definer).integer().not_null())
            .col(ColumnDef::new(PropertyDefinition::Name).string().not_null())
            .build(SqliteQueryBuilder);

        let property_def_index_create = Index::create()
            .table(PropertyDefinition::Table)
            .index_type(IndexType::Hash)
            .name("property_lookup_index")
            .col(PropertyDefinition::Definer)
            .col(PropertyDefinition::Name)
            .build(SqliteQueryBuilder);

        let pval_table_create = Table::create()
            .table(Property::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(Property::Pid)
                    .integer()
                    .primary_key()
                    .not_null(),
            )
            .col(ColumnDef::new(Property::Owner).integer().not_null())
            .col(ColumnDef::new(Property::Flags).integer().not_null())
            .col(ColumnDef::new(Property::Value).integer().not_null())
            .foreign_key(
                ForeignKey::create()
                    .on_delete(ForeignKeyAction::Cascade)
                    .from_col(Property::Pid)
                    .to_col(PropertyDefinition::Pid)
                    .to_tbl(PropertyDefinition::Table),
            )
            .build(SqliteQueryBuilder);

        self.tx.execute_batch(
            &[
                object_table_create,
                property_def_table_create,
                property_def_index_create,
                pval_table_create,
            ]
            .join(";"),
        )?;
        Ok(())
    }
}


// TODO translate -1 to and from null
impl<'a> Objects for SQLiteTx<'a> {
    fn create_object(&mut self) -> Result<Objid, Error> {
        let nullobj: Option<i64> = None;
        let (insert_sql, values) = Query::insert()
            .into_table(Object::Table)
            .columns([
                Object::Owner,
                Object::Parent,
                Object::Location,
                Object::Name,
                Object::Flags,
            ])
            .values_panic([
                nullobj.into(),
                nullobj.into(),
                nullobj.into(),
                "".into(),
                0.into(),
            ])
            .build_rusqlite(SqliteQueryBuilder);

        let result = self.tx.execute(&insert_sql, &*values.as_params())?;
        // TODO replace with proper error handling
        assert_eq!(result, 1);
        let oid = self.tx.last_insert_rowid();
        Ok(Objid(oid))
    }

    fn destroy_object(&mut self, oid: Objid) -> Result<(), Error> {
        let (delete_sql, values) = Query::delete()
            .from_table(Object::Table)
            .cond_where(Expr::col(Object::Oid).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let result = self.tx.execute(&delete_sql, &*values.as_params())?;
        // TODO replace with proper error handling
        assert_eq!(result, 1);
        Ok(())
    }

    fn object_valid(&self, oid: Objid) -> Result<bool, Error> {
        let (count_query, values) = Query::select()
            .from(Object::Table)
            .expr(Func::count(Expr::col(Object::Oid)))
            .cond_where(Expr::col(Object::Oid).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);

        let mut query = self.tx.prepare(&count_query)?;
        let count = query.query_row(&*values.as_params(), |r| {
            let count: usize = r.get(0)?;
            Ok(count)
        })?;
        Ok(count > 0)
    }

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: EnumSet<ObjAttr>,
    ) -> Result<ObjAttrs, anyhow::Error> {
        let columns = attributes.iter().map(object_attr_to_column);
        let (query, values) = Query::select()
            .from(Object::Table)
            .columns(columns)
            .cond_where(Expr::col(Object::Oid).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);

        let mut query = self.tx.prepare(&query)?;
        let attrs = query.query_row(&*values.as_params(), |r| {
            let mut attrs = ObjAttrs {
                owner: None,
                name: None,
                parent: None,
                location: None,
                flags: None,
            };
            for (c_num, a) in attributes.iter().enumerate() {
                match a {
                    ObjAttr::Owner => attrs.owner = retr_objid(r, c_num)?,
                    ObjAttr::Name => attrs.name = Some(r.get(c_num)?),
                    ObjAttr::Parent => attrs.parent = retr_objid(r, c_num)?,
                    ObjAttr::Location => attrs.location = retr_objid(r, c_num)?,
                    ObjAttr::Flags => {
                        let u: u8 = r.get(c_num)?;
                        let e: EnumSet<ObjAttr> = EnumSet::from_u8(u);
                        attrs.flags = Some(e);
                    }
                }
            }
            Ok(attrs)
        })?;

        Ok(attrs)
    }

    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), anyhow::Error> {
        let mut params = vec![];
        if let Some(o) = attributes.location {
            params.push((Object::Location, o.0.into()));
        }
        if let Some(s) = attributes.name {
            params.push((Object::Name, s.into()))
        }
        if let Some(f) = attributes.flags {
            let u: u8 = f.as_u8();
            params.push((Object::Flags, u.into()));
        }
        if let Some(o) = attributes.owner {
            params.push((Object::Owner, o.0.into()));
        }
        if let Some(o) = attributes.parent {
            params.push((Object::Parent, o.0.into()));
        }
        let (query, values) = Query::update()
            .table(Object::Table)
            .cond_where(Expr::col(Object::Oid).eq(oid.0))
            .values(params)
            .build_rusqlite(SqliteQueryBuilder);

        let count = self.tx.execute(&query, &*values.as_params()).unwrap();
        assert_eq!(count, 1);
        Ok(())
    }

    fn count_object_children(&self, _oid: Objid) -> Result<usize, Error> {
        todo!()
    }

    fn object_children(&self, _oid: Objid) -> Result<Vec<Objid>, Error> {
        todo!()
    }

    fn count_object_contents(&self, _oid: Objid) -> Result<usize, Error> {
        todo!()
    }

    fn object_contents(&self, _oid: Objid) -> Result<Vec<Objid>, Error> {
        todo!()
    }
}

impl<'a> PropDefs for SQLiteTx<'a> {
    fn find_propdef(&mut self, target: Objid, pname: &str) -> Result<Option<Propdef>, Error> {
        todo!()
    }

    fn add_propdef(
        &mut self,
        oid: Objid,
        pname: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        val: Var,
    ) -> Result<Pid, Error> {
        let (insert_sql, values) = Query::insert()
            .into_table(PropertyDefinition::Table)
            .columns([
                PropertyDefinition::Definer,
                PropertyDefinition::Name,
            ])
            .values_panic([
                oid.0.into(),
                pname.into(),
            ])
            .build_rusqlite(SqliteQueryBuilder);
        self.tx.execute(&insert_sql, &*values.as_params())?;

        let pid = Pid(self.tx.last_insert_rowid());
        self.set_property(pid, val, owner, flags)?;
        Ok(pid)
    }

    fn rename_propdef(&mut self, _oid: Objid, old: &str, new: &str) -> Result<(), Error> {
        let (update_query, values) = Query::update()
            .table(PropertyDefinition::Table)
            .value(PropertyDefinition::Name, new)
            .and_where(Expr::col(PropertyDefinition::Name).eq(old))
            .build_rusqlite(SqliteQueryBuilder);
        let result = self.tx.execute(&update_query, &*values.as_params())?;
        // TODO proper meaningful error codes
        assert_eq!(result, 1);
        Ok(())
    }

    fn delete_propdef(&mut self, oid: Objid, pname: &str) -> Result<(), Error> {
        let (delete_sql, values) = Query::delete()
            .from_table(PropertyDefinition::Table)
            .cond_where(Expr::col(PropertyDefinition::Definer).eq(oid.0))
            .and_where(Expr::col(PropertyDefinition::Name).eq(pname))
            .build_rusqlite(SqliteQueryBuilder);
        let result = self.tx.execute(&delete_sql, &*values.as_params())?;
        // TODO proper meaningful error codes
        assert_eq!(result, 1);
        Ok(())
    }

    fn count_propdefs(&mut self, oid: Objid) -> Result<usize, Error> {
        let (count_query, values) = Query::select()
            .from(PropertyDefinition::Table)
            .expr(Func::count(Expr::col(PropertyDefinition::Definer)))
            .cond_where(Expr::col(PropertyDefinition::Definer).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);

        let mut query = self.tx.prepare(&count_query)?;
        let count = query.query_row(&*values.as_params(), |r| {
            let count: usize = r.get(0)?;
            Ok(count)
        })?;
        Ok(count)
    }

    fn get_propdefs(&mut self, oid: Objid) -> Result<Vec<Propdef>, Error> {
        let (query, values) = Query::select()
            .from(PropertyDefinition::Table)
            .columns([
                PropertyDefinition::Pid,
                PropertyDefinition::Definer,
                PropertyDefinition::Name,
            ])
            .cond_where(Expr::col(PropertyDefinition::Definer).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut query = self.tx.prepare(&query)?;
        let results = query
            .query_map(&*values.as_params(), |r| {
                let propdef = Propdef {
                    pid: Pid(r.get(0)?),
                    definer: Objid(r.get(1)?),
                    pname: r.get(2)?,
                };
                Ok(propdef)
            })
            .unwrap();
        let results = results.map(|r| r.expect("could not decode propdef tuple"));
        let results: Vec<Propdef> = results.collect();
        Ok(results)
    }
}

/// Objects either have or don't have a property value for a given definition defined by a parent.
/// If they do not have a defined value, the value is inherited from either a parent, or the
/// definition
/// So to search we run a query which:
///             checks property def for the oid/pname combination, recursively walking up the inheritance tree
///             and then joins the resulting property handle onto the property value table somehow
///
/// will have to play with recursive WITH queries & object inheritance
/// should be possible to make an ordered transitive closure up to root from a child, and ORDER and LIMIT to find the first not-null occurrence of a given pname
///

impl<'a> Properties for SQLiteTx<'a> {
    /*
 Draft for query that does transitive resolution through inheritance

 WITH RECURSIVE parents_of(n) AS (SELECT oid
                                 from object
                                 where oid = 2
                                 UNION
                                 SELECT parent
                                 FROM object,
                                      parents_of
                                 where oid = parents_of.n)
select pd.pid,
       pd.oid   as definer,
       pd.name  as name,
       p.owner  as location,
       p.value  as value
from property_definition pd
         left outer join property p on pd.pid = p.pid
where pd.name = 'test';

     */
    fn get_property(&self, handle: Pid, attrs: EnumSet<PropAttr>) -> Result<PropAttrs, Error> {
        todo!()
    }

    fn set_property(
        &self,
        handle: Pid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), Error> {
        let flags_encoded = flags.as_u8();
        let encoded_val: Vec<u8> = bincode::encode_to_vec(&value, self.bincode_cfg).unwrap();

        let (query, values) = Query::insert()
            .into_table(Property::Table)
            .columns([
                Property::Pid,
                Property::Owner,
                Property::Flags,
                Property::Value,
            ])
            .values_panic([
                handle.0.into(),
                owner.0.into(),
                flags_encoded.into(),
                encoded_val.clone().into(),
            ])
            .on_conflict(
                OnConflict::new()
                    .values([
                        (Property::Owner, owner.0.into()),
                        (Property::Flags, flags_encoded.into()),
                        (Property::Value, encoded_val.into()),
                    ])
                    .action_and_where(Expr::col(Property::Pid).eq(handle.0))
                    .to_owned(),
            )
            .build_rusqlite(SqliteQueryBuilder);

        self.tx.execute(&query, &*values.as_params()).unwrap();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::sqllite::SQLiteTx;
    use crate::model::objects::{ObjAttr, ObjAttrs, Objects};
    use crate::model::props::{PropDefs, PropFlag, Propdef, Properties};
    use crate::model::var::{Objid, Var};
    use antlr_rust::CoerceTo;
    use rusqlite::Connection;

    #[test]
    fn object_create_check_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object().unwrap();
        assert!(s.object_valid(o).unwrap());
        s.destroy_object(o).unwrap();
        assert_eq!(s.object_valid(o).unwrap(), false);
        s.tx.commit().unwrap();
    }

    #[test]
    fn object_create_set_get_attrs() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object().unwrap();

        s.object_set_attrs(
            o,
            ObjAttrs {
                owner: Some(Objid(66)),
                name: Some(String::from("test")),
                parent: None,
                location: None,
                flags: None,
            },
        )
        .unwrap();

        let attrs = s
            .object_get_attrs(
                o,
                ObjAttr::Flags
                    | ObjAttr::Location
                    | ObjAttr::Parent
                    | ObjAttr::Owner
                    | ObjAttr::Name,
            )
            .unwrap();

        assert_eq!(attrs.name.unwrap(), "test");
        s.tx.commit().unwrap();
    }

    #[test]
    fn propdef_create_get_update_count_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object().unwrap();

        let pid = s.add_propdef(
            o,
            "test",
            o,
            PropFlag::Chown | PropFlag::Read,
            Var::Str(String::from("testing")),
        )
        .unwrap();

        let pds = s.get_propdefs(o).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].definer, o);
        assert_eq!(pds[0].pname, "test");
        assert_eq!(pds[0].pid, pid);

        s.rename_propdef(o, "test", "test2").unwrap();

        s.set_property(
            pds[0].pid,
            Var::Str(String::from("testing")),
            o,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        let c = s.count_propdefs(o).unwrap();
        assert_eq!(c, 1);

        s.delete_propdef(o, "test2").unwrap();

        let c = s.count_propdefs(o).unwrap();
        assert_eq!(c, 0);
        s.tx.commit().unwrap();
    }

    #[test]
    fn property_inheritance() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let parent = s.create_object().unwrap();
        let child1 = s.create_object().unwrap();
        let child2 = s.create_object().unwrap();
        s.object_set_attrs(child1, ObjAttrs {
            owner: None,
            name: None,
            parent: Some(parent),
            location: None,
            flags: None
        }).unwrap();
        s.object_set_attrs(child2, ObjAttrs {
            owner: None,
            name: None,
            parent: Some(child1),
            location: None,
            flags: None
        }).unwrap();

        let pid = s.add_propdef(
            parent,
            "test",
            parent,
            PropFlag::Chown | PropFlag::Read,
            Var::Str(String::from("testing")),
        )
            .unwrap();

        let pds = s.get_propdefs(parent).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].definer, parent);
        assert_eq!(pds[0].pid, pid
                   , "test");

        s.set_property(
            pid,
            Var::Str(String::from("testing")),
            child1,
            PropFlag::Read | PropFlag::Write,
        )
            .unwrap();

        s.tx.commit().unwrap();
    }


}

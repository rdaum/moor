
use crate::model::objects::{ObjAttr, ObjAttrs, Objects};
use crate::model::props::{PropDefs, Propdef};
use crate::model::var::{Objid, Var};

use anyhow::Error;
use bincode::config::Configuration;
use bincode::{config};
use enumset::{EnumSet};
use rusqlite::{Row, Transaction};
use sea_query::{BlobSize, ColumnDef, DynIden, Expr, Func, Iden, Index, IntoIden, Query, SqliteQueryBuilder, Table};
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
    Oid,
    Name,
    Owner,
    Flags,
    Value,
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
            .col(ColumnDef::new(PropertyDefinition::Oid).integer().not_null())
            .col(ColumnDef::new(PropertyDefinition::Name).string().not_null())
            .col(ColumnDef::new(PropertyDefinition::Owner).integer())
            .col(
                ColumnDef::new(PropertyDefinition::Flags)
                    .integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(PropertyDefinition::Value)
                    .blob(BlobSize::Medium)
                    .not_null(),
            )
            .primary_key(
                Index::create()
                    .col(PropertyDefinition::Oid)
                    .col(PropertyDefinition::Name),
            )
            .build(SqliteQueryBuilder);

        self.tx
            .execute_batch(&[object_table_create, property_def_table_create].join(";"))?;
        Ok(())
    }
}

impl<'a> PropDefs for SQLiteTx<'a> {
    fn add_propdef(&mut self, propdef: Propdef) -> Result<(), Error> {
        let encoded_val: Vec<u8> = bincode::encode_to_vec(&propdef.val, self.bincode_cfg).unwrap();
        let encoded_flags: u8 = propdef.flags.as_u8();

        let (insert_sql, values) = Query::insert()
            .into_table(PropertyDefinition::Table)
            .columns([
                PropertyDefinition::Oid,
                PropertyDefinition::Name,
                PropertyDefinition::Owner,
                PropertyDefinition::Flags,
                PropertyDefinition::Value,
            ])
            .values_panic([
                propdef.oid.0.into(),
                propdef.pname.into(),
                propdef.owner.0.into(),
                encoded_flags.into(),
                encoded_val.into(),
            ])
            .build_rusqlite(SqliteQueryBuilder);
        self.tx.execute(&insert_sql, &*values.as_params())?;

        Ok(())
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
            .cond_where(Expr::col(PropertyDefinition::Oid).eq(oid.0))
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
            .expr(Func::count(Expr::col(PropertyDefinition::Oid)))
            .cond_where(Expr::col(PropertyDefinition::Oid).eq(oid.0))
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
                PropertyDefinition::Oid,
                PropertyDefinition::Name,
                PropertyDefinition::Owner,
                PropertyDefinition::Flags,
                PropertyDefinition::Value,
            ])
            .cond_where(Expr::col(PropertyDefinition::Owner).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut query = self.tx.prepare(&query)?;
        let results = query
            .query_map(&*values.as_params(), |r| {
                let flags : u8 = r.get(3)?;
                let flags = EnumSet::from_u8(flags);
                let val_bytes: Vec<u8> = r.get(4)?;
                let (val, _): (Var, usize) =
                    bincode::decode_from_slice(&val_bytes[..], self.bincode_cfg).unwrap();
                let propdef = Propdef {
                    oid: Objid(r.get(0)?),
                    pname: r.get(1)?,
                    owner: Objid(r.get(2)?),
                    flags,
                    val,
                };
                Ok(propdef)
            })
            .unwrap();
        let results = results.map(|r| r.expect("could not decode propdef tuple"));
        let results: Vec<Propdef> = results.collect();
        Ok(results)
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

#[cfg(test)]
mod tests {
    use crate::db::sqllite::SQLiteTx;
    use crate::model::objects::{ObjAttr, ObjAttrs, Objects};
    use crate::model::props::{PropDefs, PropFlag, Propdef};
    use crate::model::var::{Objid, Var};
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
    }

    #[test]
    fn propdef_create_get_update_count_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object().unwrap();

        s.add_propdef(Propdef {
            oid: o,
            pname: String::from("test"),
            owner: o,
            flags: PropFlag::Chown | PropFlag::Read,
            val: Var::Str(String::from("testing")),
        })
        .unwrap();

        let pds = s.get_propdefs(o).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].owner, o);
        assert_eq!(pds[0].pname, "test");

        s.rename_propdef(o, "test", "test2").unwrap();

        let c = s.count_propdefs(o).unwrap();
        assert_eq!(c, 1);

        s.delete_propdef(o, "test2").unwrap();

        let c = s.count_propdefs(o).unwrap();
        assert_eq!(c, 0);
    }
}

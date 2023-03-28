use anyhow::Error;
use bincode::config;
use bincode::config::Configuration;
use bytes::Bytes;
use enumset::EnumSet;
use itertools::Itertools;
use rusqlite::{Connection, Row, Transaction};
use sea_query::{
    all, Alias, BlobSize, ColumnDef, CommonTableExpression, DynIden, Expr, ForeignKey,
    ForeignKeyAction, Func, Iden, Index, IndexType, IntoCondition, IntoIden, JoinType, OnConflict,
    Query, QueryStatementWriter, SelectStatement, SimpleExpr, SqliteQueryBuilder, Table, UnionType,
    WithClause,
};
use sea_query_rusqlite::{RusqliteBinder, RusqliteValue, RusqliteValues};

use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
use crate::model::permissions::Permissions;
use crate::model::props::{
    Pid, PropAttr, PropAttrs, PropDefs, PropFlag, Propdef, Properties, PropertyInfo,
};
use crate::model::r#match::VerbArgsSpec;
use crate::model::var::{Objid, Var};
use crate::model::verbs::{Program, VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use crate::model::ObjDB;

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
    Location,
    Value,
    Owner,
    Flags,
}

#[derive(Iden)]
enum Verb {
    Table,
    Vid,
    Owner,
    Definer,
    Flags,
    ArgsSpec,
    Program,
}

#[derive(Iden)]
enum VerbName {
    Table,
    NameId,
    Vid,
    Name,
}

pub struct SQLiteTx<'a> {
    tx: Option<Transaction<'a>>,
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

fn property_attr_to_column<'a>(attr: PropAttr) -> (DynIden, DynIden) {
    match attr {
        PropAttr::Value => (Property::Table.into_iden(), Property::Value.into_iden()),
        PropAttr::Location => (Property::Table.into_iden(), Property::Location.into_iden()),
        PropAttr::Owner => (Property::Table.into_iden(), Property::Owner.into_iden()),
        PropAttr::Flags => (Property::Table.into_iden(), Property::Flags.into_iden()),
    }
}

fn verb_attr_to_column<'a>(attr: VerbAttr) -> (DynIden, DynIden) {
    match attr {
        VerbAttr::Definer => (Verb::Table.into_iden(), Verb::Definer.into_iden()),
        VerbAttr::Owner => (Verb::Table.into_iden(), Verb::Owner.into_iden()),
        VerbAttr::Flags => (Verb::Table.into_iden(), Verb::Flags.into_iden()),
        VerbAttr::ArgsSpec => (Verb::Table.into_iden(), Verb::ArgsSpec.into_iden()),
        VerbAttr::Program => (Verb::Table.into_iden(), Verb::Program.into_iden()),
    }
}

fn retr_objid(r: &Row, c_num: usize) -> Result<Option<Objid>, rusqlite::Error> {
    let x: Option<i64> = r.get(c_num)?;
    Ok(x.map(Objid))
}

fn transitive_inheritance_clause(oid: Objid) -> WithClause {
    let self_relval = SelectStatement::new()
        .expr(Expr::asterisk())
        .from_values([(oid.0)], Alias::new("oid"))
        .to_owned();

    let parents_of = Alias::new("parents_of");
    let transitive = SelectStatement::new()
        .from(Object::Table)
        .column(Object::Parent)
        .join(
            JoinType::InnerJoin,
            parents_of.clone(),
            Expr::tbl(parents_of.clone(), Alias::new("oid"))
                .equals(Object::Table, Object::Oid)
                .into_condition(),
        )
        .to_owned();

    let cte = CommonTableExpression::new()
        .query(
            self_relval
                .clone()
                .union(UnionType::All, transitive.clone())
                .to_owned(),
        )
        .column(Alias::new("oid"))
        .table_name(parents_of.clone())
        .to_owned();

    let with = Query::with().recursive(true).cte(cte).to_owned();

    with
}

struct VerbPivot {
    vid: i64,
    name: String,
    name_id: i64,
    attrs: VerbAttrs,
}

impl<'a> SQLiteTx<'a> {
    pub fn new(connection: &'a mut Connection) -> Result<Self, anyhow::Error> {
        let tx = connection.transaction()?;
        let s = Self {
            tx: Some(tx),
            bincode_cfg: config::standard(),
        };
        Ok(s)
    }

    pub fn initialize_schema(&mut self) -> Result<(), anyhow::Error> {
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
            .col(
                ColumnDef::new(PropertyDefinition::Definer)
                    .integer()
                    .not_null(),
            )
            .col(ColumnDef::new(PropertyDefinition::Name).string().not_null())
            .build(SqliteQueryBuilder);

        let property_def_index_create = Index::create()
            .if_not_exists()
            .table(PropertyDefinition::Table)
            .index_type(IndexType::Hash)
            .name("property_lookup_index")
            .col(PropertyDefinition::Definer)
            .col(PropertyDefinition::Name)
            .build(SqliteQueryBuilder);

        let pval_table_create = Table::create()
            .table(Property::Table)
            .if_not_exists()
            .col(ColumnDef::new(Property::Pid).integer().not_null())
            .col(ColumnDef::new(Property::Owner).integer().not_null())
            .col(ColumnDef::new(Property::Location).integer().not_null())
            .col(ColumnDef::new(Property::Flags).integer().not_null())
            .col(ColumnDef::new(Property::Value).integer().not_null())
            .foreign_key(
                ForeignKey::create()
                    .on_delete(ForeignKeyAction::Cascade)
                    .from_col(Property::Pid)
                    .to_col(PropertyDefinition::Pid)
                    .to_tbl(PropertyDefinition::Table),
            )
            .primary_key(Index::create().col(Property::Location).col(Property::Pid))
            .build(SqliteQueryBuilder);

        let pval_location_idx = Index::create()
            .if_not_exists()
            .table(Property::Table)
            .index_type(IndexType::Hash)
            .name("property_location_hash")
            .col(Property::Location)
            .build(SqliteQueryBuilder);

        let verb_table_create = Table::create()
            .if_not_exists()
            .table(Verb::Table)
            .col(ColumnDef::new(Verb::Vid).integer().primary_key().not_null())
            .col(ColumnDef::new(Verb::Owner).integer().not_null())
            .col(ColumnDef::new(Verb::Definer).integer().not_null())
            .col(ColumnDef::new(Verb::Flags).integer().not_null())
            .col(
                ColumnDef::new(Verb::ArgsSpec)
                    .blob(BlobSize::Tiny)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Verb::Program)
                    .blob(BlobSize::Medium)
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .on_delete(ForeignKeyAction::Cascade)
                    .from_col(Verb::Definer)
                    .to_col(Object::Oid)
                    .to_tbl(Object::Table),
            )
            .foreign_key(
                ForeignKey::create()
                    .on_delete(ForeignKeyAction::Cascade)
                    .from_col(Verb::Owner)
                    .to_col(Object::Oid)
                    .to_tbl(Object::Table),
            )
            .build(SqliteQueryBuilder);

        let verb_name_table_create = Table::create()
            .if_not_exists()
            .table(VerbName::Table)
            .col(
                ColumnDef::new(VerbName::NameId)
                    .integer()
                    .primary_key()
                    .not_null(),
            )
            .col(ColumnDef::new(VerbName::Vid).integer().not_null())
            .col(ColumnDef::new(VerbName::Name).string().not_null())
            .build(SqliteQueryBuilder);

        let vid_name_index = Index::create()
            .if_not_exists()
            .table(VerbName::Table)
            .name("verb_and_vid_idx")
            .col(VerbName::Vid)
            .index_type(IndexType::Hash)
            .col(VerbName::Name)
            .build(SqliteQueryBuilder);
        let vid_index = Index::create()
            .if_not_exists()
            .table(VerbName::Table)
            .name("verb_name_idx")
            .col(VerbName::Vid)
            .index_type(IndexType::BTree)
            .build(SqliteQueryBuilder);

        self.tx.as_mut().unwrap().execute_batch(
            &[
                object_table_create,
                property_def_table_create,
                property_def_index_create,
                pval_table_create,
                pval_location_idx,
                verb_table_create,
                verb_name_table_create,
                vid_index,
                vid_name_index,
            ]
            .join(";"),
        )?;
        Ok(())
    }

    fn verb_attrs_from_result(
        &self,
        r: &Row,
        req_attrs: EnumSet<VerbAttr>,
    ) -> rusqlite::Result<VerbPivot> {
        let vid: i64 = r.get("vid")?;
        let name: String = r.get("name")?;
        let name_id: i64 = r.get("name_id")?;

        let mut attrs = VerbAttrs {
            definer: None,
            owner: None,
            flags: None,
            args_spec: None,
            program: None,
        };
        for (c_num, a) in req_attrs.iter().enumerate() {
            match a {
                VerbAttr::Definer => attrs.definer = retr_objid(r, c_num)?,
                VerbAttr::Owner => attrs.owner = retr_objid(r, c_num)?,
                VerbAttr::Flags => {
                    let fe: u16 = r.get(c_num)?;
                    let flags: EnumSet<VerbFlag> = EnumSet::from_u16(fe);
                    attrs.flags = Some(flags);
                }
                VerbAttr::ArgsSpec => {
                    let args_spec_encoded: Vec<u8> = r.get(c_num)?;
                    let (decoded_val, _) =
                        bincode::decode_from_slice(&args_spec_encoded, self.bincode_cfg).unwrap();

                    attrs.args_spec = Some(decoded_val);
                }
                VerbAttr::Program => {
                    let prg_bytes: Vec<u8> = r.get(c_num)?;
                    let prg = Program(Bytes::from(prg_bytes));
                    attrs.program = Some(prg);
                }
            }
        }
        Ok(VerbPivot {
            vid,
            name,
            name_id,
            attrs,
        })
    }

    fn map_verbs(
        &self,
        results: impl Iterator<Item = VerbPivot>,
    ) -> Result<Vec<VerbInfo>, anyhow::Error> {
        let by_vid = results.group_by(|r| r.vid);

        let results = by_vid.into_iter().map(|r| {
            let mut attrs: Option<VerbAttrs> = None;
            let mut names = vec![];
            for i in r.1 {
                names.push(i.name);
                if attrs.is_none() {
                    attrs = Some(i.attrs.clone());
                }
            }
            VerbInfo {
                vid: Vid(r.0),
                names,
                attrs: attrs.unwrap(),
            }
        });

        Ok(results.collect())
    }
}

// TODO translate -1 to and from null
impl<'a> Objects for SQLiteTx<'a> {
    fn create_object(&mut self, oid: Option<Objid>, attrs: &ObjAttrs) -> Result<Objid, Error> {
        let owner = match attrs.owner {
            None => None::<i64>.into(),
            Some(o) => o.0.into(),
        };
        let parent = match attrs.parent {
            None => None::<i64>.into(),
            Some(o) => o.0.into(),
        };
        let location = match attrs.location {
            None => None::<i64>.into(),
            Some(o) => o.0.into(),
        };
        let name = match &attrs.name {
            None => "".into(),
            Some(s) => s.as_str().into(),
        };
        let flags = match &attrs.flags {
            None => 0.into(),
            Some(f) => f.as_u8().into(),
        };

        let mut columns = vec![
            Object::Owner,
            Object::Parent,
            Object::Location,
            Object::Name,
            Object::Flags,
        ];

        let mut values: Vec<SimpleExpr> = vec![owner, parent, location, name, flags];
        if let Some(oid) = &oid {
            columns.push(Object::Oid);
            values.push(oid.0.into())
        }

        let (insert_sql, values) = Query::insert()
            .into_table(Object::Table)
            .columns(columns)
            .values_panic(values)
            .build_rusqlite(SqliteQueryBuilder);

        let result = self
            .tx
            .as_mut()
            .unwrap()
            .execute(&insert_sql, &*values.as_params())?;
        // TODO replace with proper error handling
        assert_eq!(result, 1);
        let oid = self.tx.as_mut().unwrap().last_insert_rowid();
        Ok(Objid(oid))
    }

    fn destroy_object(&mut self, oid: Objid) -> Result<(), Error> {
        let (delete_sql, values) = Query::delete()
            .from_table(Object::Table)
            .cond_where(Expr::col(Object::Oid).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let result = self
            .tx
            .as_mut()
            .unwrap()
            .execute(&delete_sql, &*values.as_params())?;
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

        let mut query = self.tx.as_ref().unwrap().prepare(&count_query)?;
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

        let mut query = self.tx.as_mut().unwrap().prepare(&query)?;
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
                        let e: EnumSet<ObjFlag> = EnumSet::from_u8(u);
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

        let count = self
            .tx
            .as_mut()
            .unwrap()
            .execute(&query, &*values.as_params())?;
        assert_eq!(count, 1);
        Ok(())
    }

    fn object_children(&self, oid: Objid) -> Result<Vec<Objid>, Error> {
        let (query, params) = Query::select()
            .column(Object::Oid)
            .from(Object::Table)
            .cond_where(Expr::col(Object::Parent).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut query = self.tx.as_ref().unwrap().prepare(&query)?;
        let results = query.query_map(&*params.as_params(), |r| {
            let oid: i64 = r.get(0).unwrap();
            Ok(Objid(oid))
        })?;
        Ok(results.map(|o| o.unwrap()).collect())
    }

    fn object_contents(&self, oid: Objid) -> Result<Vec<Objid>, Error> {
        let (query, params) = Query::select()
            .column(Object::Oid)
            .from(Object::Table)
            .cond_where(Expr::col(Object::Location).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut query = self.tx.as_ref().unwrap().prepare(&query)?;
        let results = query.query_map(&*params.as_params(), |r| {
            let oid: i64 = r.get(0).unwrap();
            Ok(Objid(oid))
        })?;
        Ok(results.map(|o| o.unwrap()).collect())
    }
}

impl<'a> PropDefs for SQLiteTx<'a> {
    fn get_propdef(&mut self, target: Objid, pname: &str) -> Result<Propdef, Error> {
        let (query, values) = Query::select()
            .from(PropertyDefinition::Table)
            .columns([PropertyDefinition::Pid])
            .cond_where(Expr::col(PropertyDefinition::Definer).eq(target.0))
            .and_where(Expr::col(PropertyDefinition::Name).eq(pname))
            .build_rusqlite(SqliteQueryBuilder);

        let result = self
            .tx
            .as_mut()
            .unwrap()
            .query_row(&query, &*values.as_params(), |r| {
                Ok(Propdef {
                    pid: Pid(r.get(0)?),
                    definer: target,
                    pname: String::from(pname),
                })
            })?;

        Ok(result)
    }

    fn add_propdef(
        &mut self,
        oid: Objid,
        pname: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        val: Option<Var>,
    ) -> Result<Pid, Error> {
        let (insert_sql, values) = Query::insert()
            .into_table(PropertyDefinition::Table)
            .columns([PropertyDefinition::Definer, PropertyDefinition::Name])
            .values_panic([oid.0.into(), pname.into()])
            .build_rusqlite(SqliteQueryBuilder);
        self.tx
            .as_mut()
            .unwrap()
            .execute(&insert_sql, &*values.as_params())?;

        let pid = Pid(self.tx.as_mut().unwrap().last_insert_rowid());
        if let Some(val) = val {
            self.set_property(pid, oid, val, owner, flags)?;
        }
        Ok(pid)
    }

    fn rename_propdef(&mut self, _oid: Objid, old: &str, new: &str) -> Result<(), Error> {
        let (update_query, values) = Query::update()
            .table(PropertyDefinition::Table)
            .value(PropertyDefinition::Name, new)
            .and_where(Expr::col(PropertyDefinition::Name).eq(old))
            .build_rusqlite(SqliteQueryBuilder);
        let result = self
            .tx
            .as_mut()
            .unwrap()
            .execute(&update_query, &*values.as_params())?;
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
        let result = self
            .tx
            .as_mut()
            .unwrap()
            .execute(&delete_sql, &*values.as_params())?;
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

        let mut query = self.tx.as_mut().unwrap().prepare(&count_query)?;
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
        let mut query = self.tx.as_mut().unwrap().prepare(&query)?;
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

impl<'a> Properties for SQLiteTx<'a> {
    fn find_property(
        &self,
        oid: Objid,
        name: &str,
        attributes: EnumSet<PropAttr>,
    ) -> Result<Option<PropertyInfo>, Error> {
        let with = transitive_inheritance_clause(oid);
        let parents_of = Alias::new("parents_of");

        let mut columns: Vec<_> = attributes.iter().map(property_attr_to_column).collect();
        columns.push((Property::Table.into_iden(), Property::Pid.into_iden()));

        let query = Query::select()
            .columns(columns)
            .from(parents_of.clone())
            .join(
                JoinType::Join,
                Property::Table,
                all![Expr::tbl(Property::Table, Property::Location)
                    .equals(parents_of, Alias::new("oid"))],
            )
            .join(
                JoinType::Join,
                PropertyDefinition::Table,
                all![Expr::tbl(Property::Table, Property::Pid)
                    .equals(PropertyDefinition::Table, PropertyDefinition::Pid),],
            )
            .cond_where(Expr::col((PropertyDefinition::Table, PropertyDefinition::Name)).eq(name))
            .to_owned();

        let query = query.with(with);

        let (query, values) = query.build(SqliteQueryBuilder);
        let mut query = self.tx.as_ref().unwrap().prepare(&query)?;

        let values = RusqliteValues(values.into_iter().map(RusqliteValue).collect());
        let mut results = query
            .query_map(&*values.as_params(), |r| {
                let mut ret_attrs = PropAttrs {
                    value: None,
                    location: None,
                    owner: None,
                    flags: None,
                };
                let pid = Pid(r.get("pid")?);
                for (c_num, a) in attributes.iter().enumerate() {
                    match a {
                        PropAttr::Owner => {
                            ret_attrs.owner = retr_objid(r, c_num)?;
                        }
                        PropAttr::Location => {
                            ret_attrs.location = retr_objid(r, c_num)?;
                        }
                        PropAttr::Value => {
                            let val_encoded: Vec<u8> = r.get(c_num)?;

                            let (decoded_val, _) =
                                bincode::serde::decode_from_slice(&val_encoded, self.bincode_cfg)
                                    .unwrap();

                            ret_attrs.value = Some(decoded_val);
                        }
                        PropAttr::Flags => {
                            let u: u8 = r.get(c_num)?;
                            let e: EnumSet<PropFlag> = EnumSet::from_u8(u);
                            ret_attrs.flags = Some(e);
                        }
                    }
                }
                Ok(PropertyInfo {
                    pid,
                    attrs: ret_attrs,
                })
            })
            .unwrap();

        match results.next() {
            None => Ok(None),
            Some(r) => Ok(Some(r?)),
        }
    }

    fn get_property(
        &self,
        oid: Objid,
        handle: Pid,
        attributes: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        let with = transitive_inheritance_clause(oid);
        let parents_of = Alias::new("parents_of");

        let columns = attributes.iter().map(property_attr_to_column);
        let query = Query::select()
            .columns(columns)
            .from(parents_of.clone())
            .join(
                JoinType::Join,
                Property::Table,
                all![Expr::tbl(Property::Table, Property::Location)
                    .equals(parents_of, Alias::new("oid"))],
            )
            .join(
                JoinType::Join,
                PropertyDefinition::Table,
                all![Expr::tbl(Property::Table, Property::Pid)
                    .equals(PropertyDefinition::Table, PropertyDefinition::Pid),],
            )
            .cond_where(Expr::col((Property::Table, Property::Pid)).eq(handle.0))
            .to_owned();

        let query = query.with(with);

        let (query, values) = query.build(SqliteQueryBuilder);
        let mut query = self.tx.as_ref().unwrap().prepare(&query)?;

        let values = RusqliteValues(values.into_iter().map(RusqliteValue).collect());
        let mut results = query
            .query_map(&*values.as_params(), |r| {
                let mut ret_attrs = PropAttrs {
                    value: None,
                    location: None,
                    owner: None,
                    flags: None,
                };
                for (c_num, a) in attributes.iter().enumerate() {
                    match a {
                        PropAttr::Owner => {
                            ret_attrs.owner = retr_objid(r, c_num)?;
                        }
                        PropAttr::Location => {
                            ret_attrs.location = retr_objid(r, c_num)?;
                        }
                        PropAttr::Value => {
                            let val_encoded: Vec<u8> = r.get(c_num)?;

                            let (decoded_val, _) =
                                bincode::serde::decode_from_slice(&val_encoded, self.bincode_cfg)
                                    .unwrap();

                            ret_attrs.value = Some(decoded_val);
                        }
                        PropAttr::Flags => {
                            let u: u8 = r.get(c_num)?;
                            let e: EnumSet<PropFlag> = EnumSet::from_u8(u);
                            ret_attrs.flags = Some(e);
                        }
                    }
                }
                Ok(ret_attrs)
            })
            .unwrap();

        match results.next() {
            None => Ok(None),
            Some(r) => Ok(Some(r?)),
        }
    }

    fn set_property(
        &self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), Error> {
        let flags_encoded = flags.as_u8();
        let encoded_val: Vec<u8> = bincode::serde::encode_to_vec(&value, self.bincode_cfg).unwrap();

        let (query, values) = Query::insert()
            .into_table(Property::Table)
            .columns([
                Property::Pid,
                Property::Location,
                Property::Owner,
                Property::Flags,
                Property::Value,
            ])
            .values_panic([
                handle.0.into(),
                location.0.into(),
                owner.0.into(),
                flags_encoded.into(),
                encoded_val.clone().into(),
            ])
            .on_conflict(
                OnConflict::new()
                    .values([
                        (Property::Location, location.0.into()),
                        (Property::Owner, owner.0.into()),
                        (Property::Flags, flags_encoded.into()),
                        (Property::Value, encoded_val.into()),
                    ])
                    .action_and_where(
                        Expr::col(Property::Pid)
                            .eq(handle.0)
                            .and(Expr::col(Property::Location).eq(location.0)),
                    )
                    .to_owned(),
            )
            .build_rusqlite(SqliteQueryBuilder);

        self.tx
            .as_ref()
            .unwrap()
            .execute(&query, &*values.as_params())
            .unwrap();
        Ok(())
    }
}

impl<'a> Verbs for SQLiteTx<'a> {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: EnumSet<VerbFlag>,
        argspec: VerbArgsSpec,
        program: Program,
    ) -> Result<crate::model::verbs::VerbInfo, Error> {
        let argspec_encoded = bincode::encode_to_vec(argspec, self.bincode_cfg).unwrap();
        let flags_encoded: SimpleExpr = flags.as_u16().into();
        let (insert, values) = Query::insert()
            .into_table(Verb::Table)
            .columns([
                Verb::Definer,
                Verb::Owner,
                Verb::Flags,
                Verb::ArgsSpec,
                Verb::Program,
            ])
            .values_panic([
                oid.0.into(),
                owner.0.into(),
                flags_encoded,
                argspec_encoded.as_slice().into(),
                program.0[..].into(),
            ])
            .build_rusqlite(SqliteQueryBuilder);

        self.tx
            .as_mut()
            .unwrap()
            .execute(&insert, &*values.as_params())?;
        let vid = self.tx.as_mut().unwrap().last_insert_rowid();
        let mut insert = Query::insert()
            .into_table(VerbName::Table)
            .columns([VerbName::Vid, VerbName::Name])
            .to_owned();

        for name in &names {
            let name = *name;
            insert.values_panic([vid.into(), name.into()]);
        }
        let (insert, values) = insert.build_rusqlite(SqliteQueryBuilder);
        self.tx
            .as_mut()
            .unwrap()
            .execute(&insert, &*values.as_params())?;

        Ok(VerbInfo {
            vid: Vid(vid),
            names: names.into_iter().map(String::from).collect(),
            attrs: VerbAttrs {
                definer: Some(oid),
                owner: Some(owner),
                flags: Some(flags),
                args_spec: Some(argspec),
                program: Some(program),
            },
        })
    }

    fn get_verbs(
        &self,
        oid: Objid,
        attributes: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Vec<crate::model::verbs::VerbInfo>, Error> {
        let mut columns: Vec<_> = attributes.iter().map(verb_attr_to_column).collect();
        columns.push((Verb::Table.into_iden(), Verb::Vid.into_iden()));
        columns.push((VerbName::Table.into_iden(), VerbName::Name.into_iden()));
        columns.push((VerbName::Table.into_iden(), VerbName::NameId.into_iden()));
        let (query, values) = Query::select()
            .from(Verb::Table)
            .columns(columns)
            .join(
                JoinType::Join,
                VerbName::Table,
                Expr::tbl(Verb::Table, Verb::Vid)
                    .equals(VerbName::Table, VerbName::Vid)
                    .into_condition(),
            )
            .cond_where(Expr::col(Verb::Definer).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.tx.as_ref().unwrap().prepare(&query)?;
        let results = stmt.query_map(&*values.as_params(), |r| {
            self.verb_attrs_from_result(r, attributes)
        })?;
        let results = results.map(|v| v.unwrap());

        self.map_verbs(results)
    }

    fn get_verb(
        &self,
        vid: Vid,
        attributes: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<crate::model::verbs::VerbInfo, Error> {
        let mut columns: Vec<_> = attributes.iter().map(verb_attr_to_column).collect();
        columns.push((Verb::Table.into_iden(), Verb::Vid.into_iden()));
        columns.push((VerbName::Table.into_iden(), VerbName::Name.into_iden()));
        columns.push((VerbName::Table.into_iden(), VerbName::NameId.into_iden()));
        let (query, values) = Query::select()
            .from(Verb::Table)
            .columns(columns)
            .join(
                JoinType::Join,
                VerbName::Name,
                Expr::tbl(Verb::Table, Verb::Vid)
                    .equals(VerbName::Table, VerbName::Vid)
                    .into_condition(),
            )
            .cond_where(Expr::col(Verb::Vid).eq(vid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.tx.as_ref().unwrap().prepare(&query)?;
        let results = stmt.query_map(&*values.as_params(), |r| {
            self.verb_attrs_from_result(r, attributes)
        })?;
        let results = results.map(|v| v.unwrap());

        match self.map_verbs(results) {
            Ok(rv) => Ok(rv[0].clone()),
            Err(e) => Err(e),
        }
    }

    fn update_verb(&self, _vid: Vid, _attrs: VerbAttrs) -> Result<(), Error> {
        // Ho-boy this is going to be fun for multiple name support. Easiest will be just to
        // delete them all and re-add.
        todo!()
    }

    fn find_command_verb(
        &self,
        _oid: Objid,
        _verb: &str,
        _argspec: VerbArgsSpec,
        _attrs: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Option<crate::model::verbs::VerbInfo>, Error> {
        todo!()
    }

    fn find_callable_verb(
        &self,
        oid: Objid,
        verb: &str,
        attrs: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Option<crate::model::verbs::VerbInfo>, Error> {
        let with = transitive_inheritance_clause(oid);
        let parents_of = Alias::new("parents_of");
        let mut columns: Vec<_> = attrs.iter().map(verb_attr_to_column).collect();
        columns.push((Verb::Table.into_iden(), Verb::Vid.into_iden()));
        columns.push((VerbName::Table.into_iden(), VerbName::Name.into_iden()));
        columns.push((VerbName::Table.into_iden(), VerbName::NameId.into_iden()));
        let query = Query::select()
            .columns(columns)
            .from(parents_of.clone())
            .join(
                JoinType::Join,
                Verb::Table,
                all![Expr::tbl(Verb::Table, Verb::Definer).equals(parents_of, Alias::new("oid"))],
            )
            .join(
                JoinType::Join,
                VerbName::Table,
                all![Expr::tbl(VerbName::Table, VerbName::Vid).equals(Verb::Table, Verb::Vid),],
            )
            .cond_where(Expr::col((VerbName::Table, VerbName::Name)).eq(verb))
            .to_owned();

        let (query, values) = query.with(with).build(SqliteQueryBuilder);

        let mut query = self.tx.as_ref().unwrap().prepare(&query)?;
        let values = RusqliteValues(values.into_iter().map(RusqliteValue).collect());

        let results = query.query_map(&*values.as_params(), |r| {
            self.verb_attrs_from_result(r, attrs)
        })?;
        let results = results.map(|v| v.unwrap());

        let mapped = self.map_verbs(results)?;

        if mapped.is_empty() {
            return Ok(None);
        }

        Ok(Some(mapped[0].clone()))
    }

    fn find_indexed_verb(
        &self,
        _oid: Objid,
        _index: usize,
        _attrs: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Option<crate::model::verbs::VerbInfo>, Error> {
        todo!()
    }
}

impl<'a> Permissions for SQLiteTx<'a> {
    fn property_allows(
        &self,
        check_flags: EnumSet<PropFlag>,
        player: Objid,
        player_flags: EnumSet<ObjFlag>,
        prop_flags: EnumSet<PropFlag>,
        prop_owner: Objid,
    ) -> bool {
        player == prop_owner
            || prop_flags.intersection(check_flags) == check_flags
            || player_flags.contains(ObjFlag::Wizard)
    }
}

impl<'a> ObjDB for SQLiteTx<'a> {
    fn initialize(&mut self) -> Result<(), Error> {
        self.initialize_schema()
    }

    fn commit(&mut self) -> Result<(), Error> {
        let tx = std::mem::take(&mut self.tx);
        tx.unwrap().commit()?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Error> {
        let tx = std::mem::take(&mut self.tx);
        tx.unwrap().rollback()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::sqllite::SQLiteTx;
    use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
    use crate::model::props::{PropAttr, PropDefs, PropFlag, Properties};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::Var;
    use crate::model::verbs::{Program, VerbAttr, VerbFlag, Verbs};
    use crate::model::ObjDB;

    #[test]
    fn object_create_check_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let mut s = SQLiteTx::new(&mut conn).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object(None, &ObjAttrs::new()).unwrap();
        assert!(s.object_valid(o).unwrap());
        s.destroy_object(o).unwrap();
        assert_eq!(s.object_valid(o).unwrap(), false);
        s.commit().unwrap();
    }

    #[test]
    fn object_check_children_contents() {
        let mut conn = Connection::open_in_memory().unwrap();
        let mut s = SQLiteTx::new(&mut conn).unwrap();
        s.initialize_schema().unwrap();

        let o1 = s.create_object(None, ObjAttrs::new().name("test")).unwrap();
        let o2 = s
            .create_object(None, ObjAttrs::new().name("test2").location(o1).parent(o1))
            .unwrap();
        let o3 = s
            .create_object(None, ObjAttrs::new().name("test3").location(o1).parent(o1))
            .unwrap();

        let children = s.object_children(o1).unwrap();
        assert_eq!(children, vec![o2, o3]);

        let contents = s.object_contents(o1).unwrap();
        assert_eq!(contents, vec![o2, o3]);

        s.commit().unwrap();
    }
    #[test]
    fn object_create_set_get_attrs() {
        let mut conn = Connection::open_in_memory().unwrap();
        let mut s = SQLiteTx::new(&mut conn).unwrap();
        s.initialize_schema().unwrap();

        let o = s
            .create_object(
                None,
                ObjAttrs::new()
                    .name("test")
                    .flags(ObjFlag::Write | ObjFlag::Read),
            )
            .unwrap();

        let attrs = s
            .object_get_attrs(o, ObjAttr::Flags | ObjAttr::Name)
            .unwrap();

        assert_eq!(attrs.name.unwrap(), "test");
        assert!(attrs.flags.unwrap().contains(ObjFlag::Write));

        s.commit().unwrap();
    }

    #[test]
    fn propdef_create_get_update_count_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let mut s = SQLiteTx::new(&mut conn).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object(None, &ObjAttrs::new()).unwrap();

        let pid = s
            .add_propdef(
                o,
                "test",
                o,
                PropFlag::Chown | PropFlag::Read,
                Some(Var::Str(String::from("testing"))),
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
            o,
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
        s.commit().unwrap();
    }

    #[test]
    fn property_inheritance() {
        let mut conn = Connection::open_in_memory().unwrap();
        let mut s = SQLiteTx::new(&mut conn).unwrap();
        s.initialize_schema().unwrap();

        let parent = s.create_object(None, &ObjAttrs::new()).unwrap();
        let child1 = s
            .create_object(None, ObjAttrs::new().parent(parent))
            .unwrap();
        let child2 = s
            .create_object(None, ObjAttrs::new().parent(child1))
            .unwrap();

        let other_root = s.create_object(None, &ObjAttrs::new()).unwrap();
        let _other_root_child = s
            .create_object(None, ObjAttrs::new().parent(other_root))
            .unwrap();

        let pid = s
            .add_propdef(
                parent,
                "test",
                parent,
                PropFlag::Chown | PropFlag::Read,
                Some(Var::Str(String::from("testing"))),
            )
            .unwrap();

        let pds = s.get_propdefs(parent).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].definer, parent);
        assert_eq!(pds[0].pid, pid, "test");

        // Verify initially that we get the value all the way from root.
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(parent));

        // Set it on the intermediate child...
        s.set_property(
            pid,
            child1,
            Var::Str(String::from("testing")),
            parent,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(child1));

        // Finally set it on the last child...
        s.set_property(
            pid,
            child2,
            Var::Str(String::from("testing")),
            parent,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(child2));

        // Finally, use the name to look it up instead of the pid
        let v = s
            .find_property(child2, "test", PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.attrs.location, Some(child2));
        // And verify we don't get it from other root or from its child
        let v = s
            .get_property(other_root, pid, PropAttr::Value | PropAttr::Location)
            .unwrap();
        assert!(v.is_none());

        s.commit().unwrap();
    }

    #[test]
    fn verb_inheritance() {
        let mut conn = Connection::open_in_memory().unwrap();
        let mut s = SQLiteTx::new(&mut conn).unwrap();
        s.initialize_schema().unwrap();

        let parent = s.create_object(None, &ObjAttrs::new()).unwrap();
        let child1 = s
            .create_object(None, ObjAttrs::new().parent(parent))
            .unwrap();
        let child2 = s
            .create_object(None, ObjAttrs::new().parent(child1))
            .unwrap();

        let other_root = s.create_object(None, &ObjAttrs::new()).unwrap();
        let _other_root_child = s
            .create_object(None, ObjAttrs::new().parent(other_root))
            .unwrap();

        let thisnonethis = VerbArgsSpec {
            dobj: ArgSpec::This,
            prep: PrepSpec::None,
            iobj: ArgSpec::This,
        };
        let _vinfo = s
            .add_verb(
                parent,
                vec!["look_down", "look_up"],
                parent,
                VerbFlag::Exec | VerbFlag::Read,
                thisnonethis,
                Program(bytes::Bytes::new()),
            )
            .unwrap();

        let verbs = s
            .get_verbs(
                parent,
                VerbAttr::Definer | VerbAttr::Owner | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert_eq!(verbs.len(), 1);
        assert_eq!(verbs[0].attrs.definer.unwrap(), parent);
        assert_eq!(verbs[0].attrs.args_spec.unwrap(), thisnonethis);
        assert_eq!(verbs[0].attrs.owner.unwrap(), parent);
        assert_eq!(verbs[0].names.len(), 2);

        // Verify initially that we get the value all the way from root.
        let v = s
            .find_callable_verb(
                child2,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), parent);

        // Set it on the intermediate child...
        let _vinfo = s
            .add_verb(
                child1,
                vec!["look_down", "look_up"],
                parent,
                VerbFlag::Exec | VerbFlag::Read,
                thisnonethis,
                Program(bytes::Bytes::new()),
            )
            .unwrap();

        // And then verify we get it from there...
        let v = s
            .find_callable_verb(
                child2,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), child1);

        // Finally set it on the last child...
        let _vinfo = s
            .add_verb(
                child2,
                vec!["look_down", "look_up"],
                parent,
                VerbFlag::Exec | VerbFlag::Read,
                thisnonethis,
                Program(bytes::Bytes::new()),
            )
            .unwrap();

        // And then verify we get it from there...
        let v = s
            .find_callable_verb(
                child2,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), child2);

        // And verify we don't get it from other root or from its child
        let v = s
            .find_callable_verb(
                other_root,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_none());

        s.commit().unwrap();
    }
}

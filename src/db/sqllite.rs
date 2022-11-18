use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
use crate::model::props::{Pid, PropAttr, PropAttrs, PropDefs, PropFlag, Propdef, Properties};
use crate::model::var::{Objid, Var};

use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{Program, VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use anyhow::Error;
use bincode::config;
use bincode::config::Configuration;
use bytes::Bytes;
use enumset::EnumSet;
use rusqlite::{Row, Transaction};
use sea_query::QueryStatement::Insert;
use sea_query::{
    all, Alias, BlobSize, ColumnDef, CommonTableExpression, DynIden, Expr, ForeignKey,
    ForeignKeyAction, Func, Iden, Index, IndexType, IntoCondition, IntoIden, JoinType, OnConflict,
    Query, QueryStatementWriter, SelectStatement, SimpleExpr, SqliteQueryBuilder, Table, UnionType,
    Value,
};
use sea_query_rusqlite::{RusqliteBinder, RusqliteValue, RusqliteValues};

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

#[derive(Iden)]
enum Verb {
    Table,
    Vid,
    Owner,
    Definer,
    Names,
    Flags,
    ArgsSpec,
    Program,
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

fn property_attr_to_column<'a>(attr: PropAttr) -> DynIden {
    match attr {
        PropAttr::Value => Property::Value.into_iden(),
        PropAttr::Owner => Property::Owner.into_iden(),
        PropAttr::Flags => Property::Flags.into_iden(),
    }
}

fn verb_attr_to_column<'a>(attr: VerbAttr) -> DynIden {
    match attr {
        VerbAttr::Definer => Verb::Definer.into_iden(),
        VerbAttr::Names => Verb::Names.into_iden(),
        VerbAttr::Owner => Verb::Owner.into_iden(),
        VerbAttr::Flags => Verb::Flags.into_iden(),
        VerbAttr::ArgsSpec => Verb::ArgsSpec.into_iden(),
        VerbAttr::Program => Verb::Program.into_iden(),
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

        let verb_table_create = Table::create()
            .if_not_exists()
            .table(Verb::Table)
            .col(ColumnDef::new(Verb::Vid).integer().primary_key().not_null())
            .col(ColumnDef::new(Verb::Owner).integer().not_null())
            .col(ColumnDef::new(Verb::Definer).integer().not_null())
            .col(ColumnDef::new(Verb::Names).string().not_null())
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

        self.tx.execute_batch(
            &[
                object_table_create,
                property_def_table_create,
                property_def_index_create,
                pval_table_create,
                verb_table_create,
            ]
            .join(";"),
        )?;
        Ok(())
    }

    fn verb_from_result(&self, r: &Row, attributes: EnumSet<VerbAttr>, vid_col: usize) -> rusqlite::Result<VerbInfo>{
        let vid : i64 = r.get(vid_col)?;
        let mut verb_attr = VerbAttrs {
            definer: None,
            names: None,
            owner: None,
            flags: None,
            args_spec: None,
            program: None
        };
        for (c_num, a) in attributes.iter().enumerate() {
            match a {
                VerbAttr::Definer => verb_attr.definer = retr_objid(r, c_num)?,
                VerbAttr::Names => verb_attr.names = Some(r.get(c_num)?),
                VerbAttr::Owner =>verb_attr.owner = retr_objid(r, c_num)?,
                VerbAttr::Flags => {
                    let fe : u8 = r.get(c_num)?;
                    let flags : EnumSet<VerbFlag> = EnumSet::from_u8(fe);
                    verb_attr.flags = Some(flags);
                }
                VerbAttr::ArgsSpec => {
                    let args_spec_encoded: Vec<u8> = r.get(c_num)?;
                    let (decoded_val, _) =
                        bincode::decode_from_slice(&args_spec_encoded, self.bincode_cfg).unwrap();

                    verb_attr.args_spec = Some(decoded_val);
                }
                VerbAttr::Program => {
                    let prg_bytes : Vec<u8> = r.get(c_num)?;
                    let prg = Program(Bytes::from(prg_bytes));
                    verb_attr.program = Some(prg);
                }
            }
        }
        let v = VerbInfo {
            vid: Vid(vid),
            attrs: verb_attr,
        };
        Ok(v)
    }

}

// TODO translate -1 to and from null
impl<'a> Objects for SQLiteTx<'a> {
    fn create_object(&mut self, attrs: &ObjAttrs) -> Result<Objid, Error> {
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
        let (insert_sql, values) = Query::insert()
            .into_table(Object::Table)
            .columns([
                Object::Owner,
                Object::Parent,
                Object::Location,
                Object::Name,
                Object::Flags,
            ])
            .values_panic([owner, parent, location, name, flags])
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

        let count = self.tx.execute(&query, &*values.as_params())?;
        assert_eq!(count, 1);
        Ok(())
    }

    fn object_children(&self, oid: Objid) -> Result<Vec<Objid>, Error> {
        let (query, params) = Query::select()
            .column(Object::Oid)
            .from(Object::Table)
            .cond_where(Expr::col(Object::Parent).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut query = self.tx.prepare(&query)?;
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
        let mut query = self.tx.prepare(&query)?;
        let results = query.query_map(&*params.as_params(), |r| {
            let oid: i64 = r.get(0).unwrap();
            Ok(Objid(oid))
        })?;
        Ok(results.map(|o| o.unwrap()).collect())
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
            .columns([PropertyDefinition::Definer, PropertyDefinition::Name])
            .values_panic([oid.0.into(), pname.into()])
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

impl<'a> Properties for SQLiteTx<'a> {
    fn get_property(
        &self,
        oid: Objid,
        handle: Pid,
        attributes: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
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

        let columns = attributes.iter().map(property_attr_to_column);

        let with = Query::with().recursive(true).cte(cte).to_owned();
        let query = Query::select()
            .columns(columns)
            .from(PropertyDefinition::Table)
            .join(
                JoinType::LeftJoin,
                Property::Table,
                all![Expr::tbl(Property::Table, Property::Pid)
                    .equals(PropertyDefinition::Table, PropertyDefinition::Pid),],
            )
            .join(
                JoinType::LeftJoin,
                parents_of.clone(),
                all![
                    Expr::tbl(PropertyDefinition::Table, PropertyDefinition::Definer)
                        .equals(parents_of.clone(), Alias::new("oid"))
                ],
            )
            .cond_where(
                Expr::col((PropertyDefinition::Table, PropertyDefinition::Pid)).eq(handle.0),
            )
            .and_where(
                Expr::col((PropertyDefinition::Table, PropertyDefinition::Definer))
                    .equals(parents_of.clone(), Alias::new("oid")),
            )
            .to_owned();

        let query = query.with(with).to_owned();

        let (query, values) = query.build(SqliteQueryBuilder);
        println!("{}", query);
        let mut query = self.tx.prepare(&query)?;

        let values = RusqliteValues(values.into_iter().map(RusqliteValue).collect());
        let mut results = query
            .query_map(&*values.as_params(), |r| {
                let mut ret_attrs = PropAttrs {
                    value: None,
                    owner: None,
                    flags: None,
                };
                for (c_num, a) in attributes.iter().enumerate() {
                    match a {
                        PropAttr::Owner => {
                            ret_attrs.owner = retr_objid(r, c_num)?;
                        }
                        PropAttr::Value => {
                            let val_encoded: Vec<u8> = r.get(c_num)?;
                            let (decoded_val, _) =
                                bincode::decode_from_slice(&val_encoded, self.bincode_cfg).unwrap();

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

        match results.nth(0) {
            None => Ok(None),
            Some(r) => Ok(Some(r?)),
        }
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

impl<'a> Verbs for SQLiteTx<'a> {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: &str,
        owner: Objid,
        flags: EnumSet<VerbFlag>,
        argspec: VerbArgsSpec,
        program: Program,
    ) -> Result<crate::model::verbs::VerbInfo, Error> {
        let argspec_encoded = bincode::encode_to_vec(argspec, self.bincode_cfg).unwrap();
        let flags_encoded: SimpleExpr = flags.as_u8().into();
        let (insert, values) = Query::insert()
            .into_table(Verb::Table)
            .columns([
                Verb::Definer,
                Verb::Owner,
                Verb::Names,
                Verb::Flags,
                Verb::ArgsSpec,
                Verb::Program,
            ])
            .values_panic([
                oid.0.into(),
                owner.0.into(),
                names.into(),
                flags_encoded,
                argspec_encoded.as_slice().into(),
                program.0[..].into(),
            ])
            .build_rusqlite(SqliteQueryBuilder);

        self.tx.execute(&insert, &*values.as_params())?;

        let vid = self.tx.last_insert_rowid();
        Ok(VerbInfo {
            vid: Vid(vid),
            attrs: VerbAttrs {
                definer: Some(oid),
                names: Some(String::from(names)),
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
        let mut columns : Vec<_> = attributes.iter().map(verb_attr_to_column).collect();
        let vid_col = columns.len();
        columns.push(Verb::Vid.into_iden());
        let (query, values) = Query::select()
            .from(Verb::Table)
            .columns(columns)
            .cond_where(Expr::col(Verb::Definer).eq(oid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.tx.prepare(&query)?;
        let results = stmt.query_map(&*values.as_params(), |r| {
            self.verb_from_result(r, attributes, vid_col)
        })?;

        Ok(results.map(|m| m.unwrap()).collect())
    }

    fn get_verb(
        &self,
        vid: Vid,
        attributes: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<crate::model::verbs::VerbInfo, Error> {
        let mut columns : Vec<_> = attributes.iter().map(verb_attr_to_column).collect();
        let vid_col = columns.len();
        columns.push(Verb::Vid.into_iden());
        let (query, values) = Query::select()
            .from(Verb::Table)
            .columns(columns)
            .cond_where(Expr::col(Verb::Vid).eq(vid.0))
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.tx.prepare(&query)?;
        let result = stmt.query_row(&*values.as_params(), |r| {
            self.verb_from_result(r, attributes, vid_col)
        })?;

        Ok(result)
    }

    fn update_verb(&self, vid: Vid, attrs: VerbAttrs) -> Result<(), Error> {
        todo!()
    }

    fn find_command_verb(
        &self,
        oid: Objid,
        verb: &str,
        argspec: VerbArgsSpec,
        attrs: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Vec<crate::model::verbs::VerbInfo>, Error> {
        todo!()
    }

    fn find_callable_verb(
        &self,
        oid: Objid,
        verb: &str,
        attrs: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Vec<crate::model::verbs::VerbInfo>, Error> {
        todo!()
    }

    fn find_indexed_verb(
        &self,
        oid: Objid,
        index: usize,
        attrs: EnumSet<crate::model::verbs::VerbAttr>,
    ) -> Result<Option<crate::model::verbs::VerbInfo>, Error> {
        todo!()
    }
}
#[cfg(test)]
mod tests {
    use crate::db::sqllite::SQLiteTx;
    use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
    use crate::model::props::{PropAttr, PropDefs, PropFlag, Propdef, Properties};
    use crate::model::var::{Objid, Var};
    use antlr_rust::CoerceTo;
    use rusqlite::Connection;

    #[test]
    fn object_create_check_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object(&ObjAttrs::new()).unwrap();
        assert!(s.object_valid(o).unwrap());
        s.destroy_object(o).unwrap();
        assert_eq!(s.object_valid(o).unwrap(), false);
        s.tx.commit().unwrap();
    }

    #[test]
    fn object_check_children_contents() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o1 = s.create_object(ObjAttrs::new().name("test")).unwrap();
        let o2 = s
            .create_object(ObjAttrs::new().name("test2").location(o1).parent(o1))
            .unwrap();
        let o3 = s
            .create_object(ObjAttrs::new().name("test3").location(o1).parent(o1))
            .unwrap();

        let children = s.object_children(o1).unwrap();
        assert_eq!(children, vec![o2, o3]);

        let contents = s.object_contents(o1).unwrap();
        assert_eq!(contents, vec![o2, o3]);

        s.tx.commit().unwrap();
    }
    #[test]
    fn object_create_set_get_attrs() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s
            .create_object(
                ObjAttrs::new()
                    .name("test")
                    .flags(ObjFlag::Write | ObjFlag::Read),
            )
            .unwrap();

        let attrs = s
            .object_get_attrs(o, ObjAttr::Flags | ObjAttr::Name)
            .unwrap();

        assert_eq!(attrs.name.unwrap(), "test");
        assert!(attrs.flags.unwrap().contains(ObjFlag::Write.into()));

        s.tx.commit().unwrap();
    }

    #[test]
    fn propdef_create_get_update_count_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        let tx = conn.transaction().unwrap();
        let mut s = SQLiteTx::new(tx).unwrap();
        s.initialize_schema().unwrap();

        let o = s.create_object(&ObjAttrs::new()).unwrap();

        let pid = s
            .add_propdef(
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

        let parent = s.create_object(&ObjAttrs::new()).unwrap();
        let child1 = s.create_object(ObjAttrs::new().parent(parent)).unwrap();
        let child2 = s.create_object(ObjAttrs::new().parent(child1)).unwrap();

        let other_root = s.create_object(&ObjAttrs::new()).unwrap();
        let other_root_child = s.create_object(ObjAttrs::new().parent(other_root)).unwrap();

        let pid = s
            .add_propdef(
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
        assert_eq!(pds[0].pid, pid, "test");

        // Verify initially that we get the value all the way from root.
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Owner)
            .unwrap()
            .unwrap();
        assert_eq!(v.owner, Some(parent));

        // Set it on the intermediate child...
        s.set_property(
            pid,
            Var::Str(String::from("testing")),
            child1,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Owner)
            .unwrap()
            .unwrap();
        assert_eq!(v.owner, Some(child1));

        // Finally set it on the last child...
        s.set_property(
            pid,
            Var::Str(String::from("testing")),
            child2,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Owner)
            .unwrap()
            .unwrap();
        assert_eq!(v.owner, Some(child2));

        // And verify we don't get it from other root or from its child
        let v = s
            .get_property(other_root, pid, PropAttr::Value | PropAttr::Owner)
            .unwrap();
        assert!(v.is_none());

        s.tx.commit().unwrap();
    }
}

use bincode::{Decode, Encode};
use crossbeam_channel::{Receiver, RecvError, Sender};
use rocksdb::ColumnFamily;
use tracing::warn;

use crate::db::rocksdb::tx_db_impl::RocksDbTx;
use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::DbStorage;

use crate::model::props::PropFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::VerbFlag;
use crate::model::ObjectError;
use crate::util::bitenum::BitEnum;
use crate::var::Objid;

// Internal storage for the verb information stored in the ObjectVerbs column family, basically
// everything sans-program.
// This data is meant to be cached locally in the transaction so that subsequent verb lookups can
// be done without having to hit the database.
#[derive(Debug, Encode, Decode, Clone)]
pub(crate) struct VerbHandle {
    pub(crate) uuid: u128,
    pub(crate) definer: Objid,
    pub(crate) owner: Objid,
    pub(crate) names: Vec<String>,
    pub(crate) flags: BitEnum<VerbFlag>,
    pub(crate) args: VerbArgsSpec,
}

#[derive(Debug, Encode, Decode, Clone)]
pub(crate) struct PropHandle {
    pub(crate) uuid: u128,
    pub(crate) definer: Objid,
    pub(crate) name: String,
    pub(crate) perms: BitEnum<PropFlag>,
    pub(crate) owner: Objid,
}

fn respond<V: Send + Sync + 'static>(
    r: Sender<Result<V, ObjectError>>,
    res: Result<V, anyhow::Error>,
) -> Result<(), anyhow::Error> {
    match res {
        Ok(v) => {
            r.send(Ok(v))?;
            Ok(())
        }
        Err(e) => match e.downcast::<ObjectError>() {
            Ok(e) => {
                r.send(Err(e))?;
                Ok(())
            }
            Err(e) => Err(e.context("Error in transaction")),
        },
    }
}

pub(crate) fn run_tx_server<'a>(
    mailbox: Receiver<Message>,
    tx: rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    cf_handles: Vec<&'a ColumnFamily>,
) -> Result<(), anyhow::Error> {
    let bincode_cfg = bincode::config::standard();
    let tx = RocksDbTx {
        tx,
        cf_handles: cf_handles.clone(),
        bincode_cfg,
    };
    let (commit_result, send_c) = loop {
        let msg = match mailbox.recv() {
            Ok(msg) => msg,
            Err(e) => {
                if e == RecvError {
                    // The other end of the channel has been dropped, so we should just rollback.
                    tx.rollback()?;
                    return Ok(());
                } else {
                    return Err(e.into());
                }
            }
        };

        match msg {
            Message::CreateObject(oid, attrs, r) => {
                respond(r, tx.create_object(oid, attrs))?;
            }
            Message::GetObjectOwner(o, r) => respond(r, tx.get_object_owner(o))?,
            Message::SetObjectOwner(o, owner, r) => respond(r, tx.set_object_owner(o, owner))?,
            Message::GetParentOf(o, r) => respond(r, tx.get_object_parent(o))?,
            Message::SetParent(o, p, r) => respond(r, tx.set_object_parent(o, p))?,
            Message::GetChildrenOf(o, r) => respond(r, tx.get_object_children(o))?,
            Message::GetLocationOf(o, r) => respond(r, tx.get_object_location(o))?,
            Message::SetLocation(o, l, r) => {
                respond(r, tx.set_object_location(o, l))?;
            }
            Message::GetContentsOf(o, r) => {
                respond(r, tx.get_object_contents(o))?;
            }
            Message::GetFlagsOf(o, r) => {
                respond(r, tx.get_object_flags(o))?;
            }
            Message::SetFlags(o, f, r) => {
                respond(r, tx.set_object_flags(o, f))?;
            }
            Message::GetObjectName(o, r) => {
                respond(r, tx.get_object_name(o))?;
            }
            Message::SetObjectName(o, names, r) => {
                respond(r, tx.set_object_name(o, names))?;
            }
            Message::GetVerbs(o, r) => {
                respond(r, tx.get_object_verbs(o))?;
            }
            Message::AddVerb {
                location,
                owner,
                names,
                program,
                flags,
                args,
                reply,
            } => {
                respond(
                    reply,
                    tx.add_object_verb(location, owner, names, program, flags, args),
                )?;
            }
            Message::DeleteVerb(o, v, r) => {
                respond(r, tx.delete_object_verb(o, v))?;
            }
            // Get information about a specific verb by its unique verb ID.
            Message::GetVerb(o, v, r) => {
                respond(r, tx.get_verb(o, v))?;
            }
            Message::GetVerbByName(o, v, r) => {
                respond(r, tx.get_verb_by_name(o, v))?;
            }
            Message::GetVerbByIndex(o, i, r) => {
                respond(r, tx.get_verb_by_index(o, i))?;
            }
            Message::GetProgram(o, v, r) => {
                respond(r, tx.get_program(o, v))?;
            }
            Message::ResolveVerb(o, n, a, r) => {
                respond(r, tx.resolve_verb(o, n, a))?;
            }
            Message::RetrieveVerb(o, v, r) => {
                respond(r, tx.retrieve_verb(o, v))?;
            }
            Message::GetProperties(o, r) => {
                respond(r, tx.get_properties(o))?;
            }
            Message::RetrieveProperty(o, u, r) => {
                respond(r, tx.retrieve_property(o, u))?;
            }
            Message::SetProperty(o, u, v, r) => {
                respond(r, tx.set_property_value(o, u, v))?;
            }
            Message::SetPropertyInfo {
                obj: o,
                uuid: u,
                owner,
                perms,
                new_name,
                reply: r,
            } => {
                respond(r, tx.set_property_info(o, u, owner, perms, new_name))?;
            }
            Message::DeleteProperty(o, u, r) => {
                respond(r, tx.delete_property(o, u))?;
            }
            Message::DefineProperty {
                obj: o,
                name,
                owner,
                perms,
                value,
                reply: r,
            } => {
                respond(r, tx.add_property(o, name, owner, perms, value))?;
            }
            Message::ResolveProperty(o, n, r) => {
                respond(r, tx.resolve_property(o, n))?;
            }
            Message::Valid(o, r) => {
                r.send(tx.object_valid(o)?)?;
            }
            Message::Commit(r) => {
                let commit_r = tx.commit()?;
                break (commit_r, r);
            }
            Message::Rollback(r) => {
                warn!("Rolling back transaction");
                tx.rollback()?;
                r.send(())?;
                return Ok(());
            }
        }
    };
    send_c.send(commit_result)?;
    Ok(())
}

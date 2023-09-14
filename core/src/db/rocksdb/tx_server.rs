/// Receives db messages over a crossbeam channel, and passes them through to the database.
use anyhow::bail;
use crossbeam_channel::{Receiver, RecvError};
use metrics_macros::increment_counter;
use rocksdb::ColumnFamily;
use tracing::warn;

use moor_values::model::WorldStateError;

use crate::db::db_message::DbMessage;
use crate::db::rocksdb::tx_db_impl::RocksDbTx;

fn respond<V: Send + Sync + 'static>(
    r: tokio::sync::oneshot::Sender<Result<V, WorldStateError>>,
    res: Result<V, anyhow::Error>,
) -> Result<(), anyhow::Error> {
    match res {
        Ok(v) => {
            let Ok(_) = r.send(Ok(v)) else {
                bail!("Failed to send response to transaction server");
            };
            Ok(())
        }
        Err(e) => match e.downcast::<WorldStateError>() {
            Ok(e) => {
                let Ok(_) = r.send(Err(e)) else {
                    bail!("Failed to send response to transaction server");
                };
                Ok(())
            }
            Err(e) => Err(e.context("Error in transaction")),
        },
    }
}

#[tracing::instrument(skip(mailbox, tx, cf_handles))]
pub(crate) fn run_tx_server<'a>(
    mailbox: Receiver<DbMessage>,
    tx: rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    cf_handles: Vec<&'a ColumnFamily>,
) -> Result<(), anyhow::Error> {
    let tx = RocksDbTx {
        tx,
        cf_handles: cf_handles.clone(),
    };
    increment_counter!("rocksdb.tx.start");
    let (commit_result, commit_response_send) = loop {
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
            DbMessage::CreateObject {
                id: oid,
                attrs,
                reply: r,
            } => {
                respond(r, tx.create_object(oid, attrs))?;
            }
            DbMessage::RecycleObject(oid, r) => {
                respond(r, tx.recycle_object(oid))?;
            }
            DbMessage::GetObjectOwner(o, r) => respond(r, tx.get_object_owner(o))?,
            DbMessage::SetObjectOwner(o, owner, r) => respond(r, tx.set_object_owner(o, owner))?,
            DbMessage::GetParentOf(o, r) => respond(r, tx.get_object_parent(o))?,
            DbMessage::SetParent(o, p, r) => respond(r, tx.set_object_parent(o, p))?,
            DbMessage::GetChildrenOf(o, r) => respond(r, tx.get_object_children(o))?,
            DbMessage::GetLocationOf(o, r) => respond(r, tx.get_object_location(o))?,
            DbMessage::SetLocationOf(o, l, r) => {
                respond(r, tx.set_object_location(o, l))?;
            }
            DbMessage::GetContentsOf(o, r) => {
                respond(r, tx.get_object_contents(o))?;
            }
            DbMessage::GetObjectFlagsOf(o, r) => {
                respond(r, tx.get_object_flags(o))?;
            }
            DbMessage::SetObjectFlagsOf(o, f, r) => {
                respond(r, tx.set_object_flags(o, f))?;
            }
            DbMessage::GetObjectNameOf(o, r) => {
                respond(r, tx.get_object_name(o))?;
            }
            DbMessage::SetObjectNameOf(o, names, r) => {
                respond(r, tx.set_object_name(o, names))?;
            }
            DbMessage::GetMaxObject(r) => {
                respond(r, tx.get_max_object())?;
            }
            DbMessage::GetVerbs(o, r) => {
                respond(r, tx.get_object_verbs(o))?;
            }
            DbMessage::AddVerb {
                location,
                owner,
                names,
                binary,
                binary_type,
                flags,
                args,
                reply,
            } => {
                respond(
                    reply,
                    tx.add_object_verb(location, owner, names, binary, binary_type, flags, args),
                )?;
            }
            DbMessage::DeleteVerb {
                location: o,
                uuid: v,
                reply: r,
            } => {
                respond(r, tx.delete_object_verb(o, v))?;
            }
            DbMessage::GetVerbByName(o, v, r) => {
                respond(r, tx.get_verb_by_name(o, v))?;
            }
            DbMessage::GetVerbByIndex(o, i, r) => {
                respond(r, tx.get_verb_by_index(o, i))?;
            }
            DbMessage::GetVerbBinary(o, v, r) => {
                respond(r, tx.get_binary(o, v))?;
            }
            DbMessage::ResolveVerb {
                location: o,
                name: n,
                argspec: a,
                reply: r,
            } => {
                respond(r, tx.resolve_verb(o, n, a))?;
            }
            DbMessage::GetProperties(o, r) => {
                respond(r, tx.get_propdefs(o))?;
            }
            DbMessage::RetrieveProperty(o, u, r) => {
                respond(r, tx.retrieve_property(o, u))?;
            }
            DbMessage::UpdateVerbDef {
                obj,
                uuid,
                names,
                owner,
                binary_type,
                args,
                flags,
                reply,
            } => {
                respond(
                    reply,
                    tx.set_verb_info(obj, uuid, owner, flags, names, args, binary_type),
                )?;
            }
            DbMessage::SetVerbBinary {
                obj,
                uuid,
                binary,
                reply,
            } => {
                respond(reply, tx.set_verb_binary(obj, uuid, binary))?;
            }

            DbMessage::SetProperty(o, u, v, r) => {
                respond(r, tx.set_property_value(o, u, v))?;
            }
            DbMessage::SetPropertyInfo {
                obj: o,
                uuid: u,
                new_owner: owner,
                new_flags: perms,
                new_name,
                reply: r,
            } => {
                respond(r, tx.set_property_info(o, u, owner, perms, new_name))?;
            }
            DbMessage::DeleteProperty(o, u, r) => {
                respond(r, tx.delete_property(o, u))?;
            }
            DbMessage::ClearProperty(o, u, r) => {
                respond(r, tx.clear_property(o, u))?;
            }
            DbMessage::DefineProperty {
                definer,
                location,
                name,
                owner,
                perms,
                value,
                reply: r,
            } => {
                respond(
                    r,
                    tx.define_property(definer, location, name, owner, perms, value),
                )?;
            }
            DbMessage::ResolveProperty(o, n, r) => {
                respond(r, tx.resolve_property(o, n))?;
            }
            DbMessage::Valid(o, r) => {
                let Ok(_) = r.send(tx.object_valid(o)?) else {
                    bail!("Could not send result")
                };
            }
            DbMessage::Commit(r) => {
                let commit_r = tx.commit()?;
                increment_counter!("rocksdb.tx.commit");
                break (commit_r, r);
            }
            DbMessage::Rollback(r) => {
                warn!("Rolling back transaction");
                tx.rollback()?;
                increment_counter!("rocksdb.tx.rollback");
                let Ok(_) = r.send(()) else {
                    bail!("Could not send result")
                };
                return Ok(());
            }
        }
    };
    let Ok(_) = commit_response_send.send(commit_result) else {
        bail!("Could not send result")
    };
    Ok(())
}

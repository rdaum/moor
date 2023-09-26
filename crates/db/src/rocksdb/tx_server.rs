/// Receives db messages over a crossbeam channel, and passes them through to the database.
use crossbeam_channel::{Receiver, RecvError};
use metrics_macros::increment_counter;
use rocksdb::ColumnFamily;
use tracing::{error, warn};

use moor_values::model::WorldStateError;

use crate::db_message::DbMessage;
use crate::rocksdb::tx_db_impl::RocksDbTx;

fn respond<V: Send + Sync + 'static>(
    r: tokio::sync::oneshot::Sender<Result<V, WorldStateError>>,
    res: Result<V, WorldStateError>,
) -> Result<(), WorldStateError> {
    match res {
        Ok(v) => {
            let Ok(_) = r.send(Ok(v)) else {
                panic!("Failed to send response to transaction server");
            };
            Ok(())
        }
        Err(e) => {
            let Ok(_) = r.send(Err(e)) else {
                panic!("Failed to send response to transaction server");
            };
            Ok(())
        }
    }
}

#[tracing::instrument(skip(mailbox, tx, cf_handles))]
pub(crate) fn run_tx_server<'a>(
    mailbox: Receiver<DbMessage>,
    tx: rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    cf_handles: Vec<&'a ColumnFamily>,
) -> Result<(), WorldStateError> {
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
                    panic!("Error receiving message from transaction server: {:?}", e);
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
                if let Err(e) = r.send(tx.object_valid(o)?) {
                    warn!(e, "Could not send result for object validity check");
                    continue;
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
                if let Err(e) = r.send(()) {
                    warn!(error = ?e, "Could not send result for rollback");
                    return Err(WorldStateError::DatabaseError(
                        "Could not send result for rollback".to_string(),
                    ));
                };
                return Ok(());
            }
        }
    };
    if let Err(e) = commit_response_send.send(commit_result) {
        error!(error = ?e, "Could not send result");
        return Err(WorldStateError::DatabaseError(
            "Could not send result for transaction".to_string(),
        ));
    };
    Ok(())
}

// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    config::Config,
    tasks::world_state_action::{WorldStateAction, WorldStateResult},
};
use moor_common::model::ObjAttrs;
use moor_common::{
    matching::{
        ObjectNameMatcher, complex_object_matcher::ComplexObjectNameMatcher,
        ws_match_env::WsMatchEnv,
    },
    model::{CommitResult, HasUuid, ObjectRef, ValSet, VerbAttrs, WorldState, WorldStateError},
    tasks::{
        CommandError, SchedulerError,
        SchedulerError::{CommandExecutionError, VerbProgramFailed},
        VerbProgramError,
    },
};
use moor_compiler::{compile, program_to_tree, unparse};
use moor_var::{E_INVIND, Obj, SYSTEM_OBJECT, program::ProgramType, v_err, v_obj};
use std::sync::Arc;

/// Executes WorldStateActions within a transaction.
/// Takes ownership of a transaction and executes actions within it.
pub struct WorldStateActionExecutor {
    tx: Box<dyn WorldState>,
    config: Arc<Config>,
}

impl WorldStateActionExecutor {
    pub fn new(tx: Box<dyn WorldState>, config: Arc<Config>) -> Self {
        Self { tx, config }
    }

    /// Execute a batch of WorldStateActions within the transaction.
    pub fn execute_batch(
        mut self,
        actions: Vec<WorldStateAction>,
        rollback: bool,
    ) -> Result<Vec<WorldStateResult>, SchedulerError> {
        let mut results = Vec::new();
        for action in actions {
            let result = self.execute_action(action)?;
            results.push(result);
        }

        // Commit or rollback the transaction
        if rollback {
            self.tx.rollback().ok();
        } else {
            match self.tx.commit() {
                Ok(CommitResult::Success { .. }) => {}
                Ok(CommitResult::ConflictRetry { conflict_info }) => {
                    let message = match conflict_info {
                        Some(info) => format!("Transaction conflict: {}", info),
                        None => "Transaction conflict".to_string(),
                    };
                    return Err(CommandExecutionError(CommandError::DatabaseError(
                        WorldStateError::DatabaseError(message),
                    )));
                }
                Err(e) => {
                    return Err(CommandExecutionError(CommandError::DatabaseError(e)));
                }
            }
        }

        Ok(results)
    }

    /// Execute a single action within the transaction.
    fn execute_action(
        &mut self,
        action: WorldStateAction,
    ) -> Result<WorldStateResult, SchedulerError> {
        match action {
            WorldStateAction::ProgramVerb {
                player,
                perms,
                obj,
                verb_name,
                code,
            } => {
                let object = match_object_ref(&player, &perms, &obj, self.tx.as_mut())
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                // Use get_verb here to avoid requiring the exec flag (bf_verb_code and editing tools
                // should be able to program verbs that are readable but not executable).
                let verbdef =
                    self.tx
                        .get_verb(&perms, &object, verb_name)
                        .map_err(|e| match e {
                            WorldStateError::VerbPermissionDenied => {
                                VerbProgramFailed(VerbProgramError::PermissionDenied)
                            }
                            _ => VerbProgramFailed(VerbProgramError::NoVerbToProgram),
                        })?;

                let program = compile(
                    code.join("\n").as_str(),
                    self.config.features.compile_options(),
                )
                .map_err(|e| VerbProgramFailed(VerbProgramError::CompilationError(e)))?;

                // Now we can update the verb.
                let update_attrs = VerbAttrs {
                    definer: None,
                    owner: None,
                    names: None,
                    flags: None,
                    args_spec: None,
                    program: Some(ProgramType::MooR(program)),
                };
                self.tx
                    .update_verb_with_id(&perms, &object, verbdef.uuid(), update_attrs)
                    .map_err(|e| match e {
                        WorldStateError::VerbPermissionDenied => {
                            VerbProgramFailed(VerbProgramError::PermissionDenied)
                        }
                        _ => VerbProgramFailed(VerbProgramError::DatabaseError),
                    })?;

                Ok(WorldStateResult::VerbProgrammed {
                    object,
                    verb: verb_name,
                })
            }

            WorldStateAction::RequestSystemProperty {
                player: _,
                obj,
                property,
            } => {
                let object =
                    match_object_ref(&SYSTEM_OBJECT, &SYSTEM_OBJECT, &obj, self.tx.as_mut())
                        .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                let value = self
                    .tx
                    .retrieve_property(&SYSTEM_OBJECT, &object, property)
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                Ok(WorldStateResult::SystemProperty(value))
            }

            WorldStateAction::RequestProperties {
                player: _,
                perms,
                obj,
                inherited,
            } => {
                let object = match_object_ref(&perms, &perms, &obj, self.tx.as_mut())
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                let mut props = Vec::new();

                if inherited {
                    // Get full inheritance chain including self
                    let ancestors = self
                        .tx
                        .ancestors_of(&perms, &object, true)
                        .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                    // Collect properties from all ancestors (including self)
                    for ancestor in ancestors.iter() {
                        // Skip ancestors that error during retrieval (permission denied, etc)
                        let Ok(ancestor_properties) = self.tx.properties(&perms, &ancestor) else {
                            continue;
                        };

                        for prop in ancestor_properties.iter() {
                            // Skip properties that error during info retrieval
                            let Ok((info, prop_perms)) =
                                self.tx.get_property_info(&perms, &ancestor, prop.name())
                            else {
                                continue;
                            };
                            props.push((info, prop_perms));
                        }
                    }
                } else {
                    // Just get properties directly defined on this object
                    let properties = self
                        .tx
                        .properties(&perms, &object)
                        .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                    for prop in properties.iter() {
                        let (info, prop_perms) = self
                            .tx
                            .get_property_info(&perms, &object, prop.name())
                            .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;
                        props.push((info, prop_perms));
                    }
                }

                Ok(WorldStateResult::Properties(props))
            }

            WorldStateAction::RequestProperty {
                player,
                perms,
                obj,
                property,
            } => {
                let object = match_object_ref(&player, &perms, &obj, self.tx.as_mut())
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                let value = self
                    .tx
                    .retrieve_property(&player, &object, property)
                    .map_err(SchedulerError::PropertyRetrievalFailed)?;

                let (info, prop_perms) = self
                    .tx
                    .get_property_info(&perms, &object, property)
                    .map_err(SchedulerError::PropertyRetrievalFailed)?;

                Ok(WorldStateResult::Property(info, prop_perms, value))
            }

            WorldStateAction::RequestVerbs {
                player: _,
                perms,
                obj,
                inherited,
            } => {
                let object = match_object_ref(&perms, &perms, &obj, self.tx.as_mut())
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                let verbs = if inherited {
                    // Get full inheritance chain including self
                    let ancestors = self
                        .tx
                        .ancestors_of(&perms, &object, true)
                        .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                    // Collect verbs from all ancestors (including self)
                    let mut all_verbs = Vec::new();
                    for ancestor in ancestors.iter() {
                        // Skip ancestors that error during retrieval (permission denied, etc)
                        let Ok(ancestor_verbs) = self.tx.verbs(&perms, &ancestor) else {
                            continue;
                        };
                        all_verbs.extend(ancestor_verbs.iter());
                    }
                    all_verbs.into_iter().collect()
                } else {
                    // Just get verbs directly defined on this object
                    self.tx
                        .verbs(&perms, &object)
                        .map_err(SchedulerError::VerbRetrievalFailed)?
                };

                Ok(WorldStateResult::Verbs(verbs))
            }

            WorldStateAction::RequestVerbCode {
                player: _,
                perms,
                obj,
                verb,
            } => {
                let object = match_object_ref(&perms, &perms, &obj, self.tx.as_mut())
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                // Use get_verb instead of find_method_verb_on to avoid exec flag requirement
                // This matches the behavior of bf_verb_code builtin
                let verbdef = self
                    .tx
                    .get_verb(&perms, &object, verb)
                    .map_err(SchedulerError::VerbRetrievalFailed)?;

                let (program, _) = self
                    .tx
                    .retrieve_verb(&perms, &object, verbdef.uuid())
                    .map_err(SchedulerError::VerbRetrievalFailed)?;

                // If the binary is empty, just return empty code
                if program.is_empty() {
                    return Ok(WorldStateResult::VerbCode(verbdef, Vec::new()));
                }

                #[allow(irrefutable_let_patterns)]
                let ProgramType::MooR(program) = program else {
                    return Err(SchedulerError::VerbRetrievalFailed(
                        WorldStateError::DatabaseError(format!(
                            "Could not decompile verb binary, expected Moo program, got {program:?}"
                        )),
                    ));
                };

                let decompiled = program_to_tree(&program).map_err(|e| {
                    SchedulerError::VerbRetrievalFailed(WorldStateError::DatabaseError(format!(
                        "Could not decompile verb binary: {e:?}"
                    )))
                })?;

                let unparsed = unparse(&decompiled, false, true).map_err(|e| {
                    SchedulerError::VerbRetrievalFailed(WorldStateError::DatabaseError(format!(
                        "Could not unparse decompiled verb: {e:?}"
                    )))
                })?;

                Ok(WorldStateResult::VerbCode(verbdef, unparsed))
            }

            WorldStateAction::ResolveObject { player, obj } => {
                let omatch = match match_object_ref(&player, &player, &obj, self.tx.as_mut()) {
                    Ok(oid) => v_obj(oid),
                    Err(WorldStateError::ObjectNotFound(_)) => v_err(E_INVIND),
                    Err(e) => return Err(SchedulerError::ObjectResolutionFailed(e)),
                };

                Ok(WorldStateResult::ResolvedObject(omatch))
            }

            WorldStateAction::RequestAllObjects { player: _ } => {
                // Get all objects - no permission check needed for listing
                let objects = self
                    .tx
                    .all_objects()
                    .map_err(SchedulerError::PropertyRetrievalFailed)?;

                Ok(WorldStateResult::AllObjects(objects.iter().collect()))
            }

            WorldStateAction::ListObjects { player } => {
                // Get all objects with metadata
                let objects = self
                    .tx
                    .all_objects()
                    .map_err(SchedulerError::PropertyRetrievalFailed)?;

                let mut object_list = Vec::new();
                for obj in objects.iter() {
                    let Ok((attrs, verbs_count, props_count)) = self.get_object(&player, &obj)
                    else {
                        continue;
                    };

                    object_list.push((obj, attrs, verbs_count, props_count));
                }

                Ok(WorldStateResult::ObjectsList(object_list))
            }

            WorldStateAction::UpdateProperty {
                player,
                perms,
                obj,
                property,
                value,
            } => {
                // Resolve the object reference
                let object = match_object_ref(&player, &perms, &obj, self.tx.as_mut())
                    .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

                // Set the property value (this will check permissions internally)
                self.tx
                    .update_property(&perms, &object, property, &value)
                    .map_err(SchedulerError::PropertyRetrievalFailed)?;

                Ok(WorldStateResult::PropertyUpdated)
            }

            WorldStateAction::GetObjectFlags { obj } => {
                // Get flags for the specified object (no permission check - flags are public)
                let flags = self
                    .tx
                    .flags_of(&obj)
                    .map_err(SchedulerError::PropertyRetrievalFailed)?;

                Ok(WorldStateResult::ObjectFlags(flags.to_u16()))
            }
        }
    }

    fn get_object(
        &mut self,
        player: &Obj,
        obj: &Obj,
    ) -> Result<(ObjAttrs, usize, usize), SchedulerError> {
        // Get individual attributes to build ObjAttrs
        let owner = self
            .tx
            .owner_of(obj)
            .map_err(SchedulerError::PropertyRetrievalFailed)?;
        let parent = self.tx.parent_of(player, obj).unwrap_or(moor_var::NOTHING);
        let location = self
            .tx
            .location_of(player, obj)
            .unwrap_or(moor_var::NOTHING);
        let flags = self
            .tx
            .flags_of(obj)
            .map_err(SchedulerError::PropertyRetrievalFailed)?;
        let name = self.tx.name_of(player, obj).unwrap_or_default();

        // Construct ObjAttrs
        let attrs = ObjAttrs::new(owner, parent, location, flags, &name);

        // Count verbs
        let verbs = self
            .tx
            .verbs(player, obj)
            .map_err(SchedulerError::VerbRetrievalFailed)?;
        let verbs_count = verbs.len();

        // Count properties
        let props = self
            .tx
            .properties(player, obj)
            .map_err(SchedulerError::PropertyRetrievalFailed)?;
        let props_count = props.len();
        Ok((attrs, verbs_count, props_count))
    }
}

/// Execute a batch of WorldStateActions against a borrowed transaction.
/// Does NOT commit or rollback — the caller is responsible for transaction lifecycle.
/// This is the entry point for task-based batch execution where the task framework
/// manages the transaction.
pub fn execute_world_state_actions(
    tx: &mut dyn WorldState,
    config: &Config,
    actions: Vec<WorldStateAction>,
) -> Result<Vec<WorldStateResult>, SchedulerError> {
    let mut results = Vec::with_capacity(actions.len());
    for action in actions {
        let result = execute_single_action(tx, config, action)?;
        results.push(result);
    }
    Ok(results)
}

/// Execute a single WorldStateAction against a borrowed transaction.
fn execute_single_action(
    tx: &mut dyn WorldState,
    config: &Config,
    action: WorldStateAction,
) -> Result<WorldStateResult, SchedulerError> {
    match action {
        WorldStateAction::ProgramVerb {
            player,
            perms,
            obj,
            verb_name,
            code,
        } => {
            let object = match_object_ref(&player, &perms, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            let verbdef = tx
                .get_verb(&perms, &object, verb_name)
                .map_err(|e| match e {
                    WorldStateError::VerbPermissionDenied => {
                        VerbProgramFailed(VerbProgramError::PermissionDenied)
                    }
                    _ => VerbProgramFailed(VerbProgramError::NoVerbToProgram),
                })?;

            let program = compile(code.join("\n").as_str(), config.features.compile_options())
                .map_err(|e| VerbProgramFailed(VerbProgramError::CompilationError(e)))?;

            let update_attrs = VerbAttrs {
                definer: None,
                owner: None,
                names: None,
                flags: None,
                args_spec: None,
                program: Some(ProgramType::MooR(program)),
            };
            tx.update_verb_with_id(&perms, &object, verbdef.uuid(), update_attrs)
                .map_err(|e| match e {
                    WorldStateError::VerbPermissionDenied => {
                        VerbProgramFailed(VerbProgramError::PermissionDenied)
                    }
                    _ => VerbProgramFailed(VerbProgramError::DatabaseError),
                })?;

            Ok(WorldStateResult::VerbProgrammed {
                object,
                verb: verb_name,
            })
        }

        WorldStateAction::RequestSystemProperty {
            player: _,
            obj,
            property,
        } => {
            let object = match_object_ref(&SYSTEM_OBJECT, &SYSTEM_OBJECT, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            let value = tx
                .retrieve_property(&SYSTEM_OBJECT, &object, property)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            Ok(WorldStateResult::SystemProperty(value))
        }

        WorldStateAction::RequestProperties {
            player: _,
            perms,
            obj,
            inherited,
        } => {
            let object = match_object_ref(&perms, &perms, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            let mut props = Vec::new();

            if inherited {
                let ancestors = tx
                    .ancestors_of(&perms, &object, true)
                    .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                for ancestor in ancestors.iter() {
                    let Ok(ancestor_properties) = tx.properties(&perms, &ancestor) else {
                        continue;
                    };

                    for prop in ancestor_properties.iter() {
                        let Ok((info, prop_perms)) =
                            tx.get_property_info(&perms, &ancestor, prop.name())
                        else {
                            continue;
                        };
                        props.push((info, prop_perms));
                    }
                }
            } else {
                let properties = tx
                    .properties(&perms, &object)
                    .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                for prop in properties.iter() {
                    let (info, prop_perms) = tx
                        .get_property_info(&perms, &object, prop.name())
                        .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;
                    props.push((info, prop_perms));
                }
            }

            Ok(WorldStateResult::Properties(props))
        }

        WorldStateAction::RequestProperty {
            player,
            perms,
            obj,
            property,
        } => {
            let object = match_object_ref(&player, &perms, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            let value = tx
                .retrieve_property(&player, &object, property)
                .map_err(SchedulerError::PropertyRetrievalFailed)?;

            let (info, prop_perms) = tx
                .get_property_info(&perms, &object, property)
                .map_err(SchedulerError::PropertyRetrievalFailed)?;

            Ok(WorldStateResult::Property(info, prop_perms, value))
        }

        WorldStateAction::RequestVerbs {
            player: _,
            perms,
            obj,
            inherited,
        } => {
            let object = match_object_ref(&perms, &perms, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            let verbs = if inherited {
                let ancestors = tx
                    .ancestors_of(&perms, &object, true)
                    .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                let mut all_verbs = Vec::new();
                for ancestor in ancestors.iter() {
                    let Ok(ancestor_verbs) = tx.verbs(&perms, &ancestor) else {
                        continue;
                    };
                    all_verbs.extend(ancestor_verbs.iter());
                }
                all_verbs.into_iter().collect()
            } else {
                tx.verbs(&perms, &object)
                    .map_err(SchedulerError::VerbRetrievalFailed)?
            };

            Ok(WorldStateResult::Verbs(verbs))
        }

        WorldStateAction::RequestVerbCode {
            player: _,
            perms,
            obj,
            verb,
        } => {
            let object = match_object_ref(&perms, &perms, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            let verbdef = tx
                .get_verb(&perms, &object, verb)
                .map_err(SchedulerError::VerbRetrievalFailed)?;

            let (program, _) = tx
                .retrieve_verb(&perms, &object, verbdef.uuid())
                .map_err(SchedulerError::VerbRetrievalFailed)?;

            if program.is_empty() {
                return Ok(WorldStateResult::VerbCode(verbdef, Vec::new()));
            }

            #[allow(irrefutable_let_patterns)]
            let ProgramType::MooR(program) = program else {
                return Err(SchedulerError::VerbRetrievalFailed(
                    WorldStateError::DatabaseError(format!(
                        "Could not decompile verb binary, expected Moo program, got {program:?}"
                    )),
                ));
            };

            let decompiled = program_to_tree(&program).map_err(|e| {
                SchedulerError::VerbRetrievalFailed(WorldStateError::DatabaseError(format!(
                    "Could not decompile verb binary: {e:?}"
                )))
            })?;

            let unparsed = unparse(&decompiled, false, true).map_err(|e| {
                SchedulerError::VerbRetrievalFailed(WorldStateError::DatabaseError(format!(
                    "Could not unparse decompiled verb: {e:?}"
                )))
            })?;

            Ok(WorldStateResult::VerbCode(verbdef, unparsed))
        }

        WorldStateAction::ResolveObject { player, obj } => {
            let omatch = match match_object_ref(&player, &player, &obj, tx) {
                Ok(oid) => v_obj(oid),
                Err(WorldStateError::ObjectNotFound(_)) => v_err(E_INVIND),
                Err(e) => return Err(SchedulerError::ObjectResolutionFailed(e)),
            };

            Ok(WorldStateResult::ResolvedObject(omatch))
        }

        WorldStateAction::RequestAllObjects { player: _ } => {
            let objects = tx
                .all_objects()
                .map_err(SchedulerError::PropertyRetrievalFailed)?;

            Ok(WorldStateResult::AllObjects(objects.iter().collect()))
        }

        WorldStateAction::ListObjects { player } => {
            let objects = tx
                .all_objects()
                .map_err(SchedulerError::PropertyRetrievalFailed)?;

            let mut object_list = Vec::new();
            for obj in objects.iter() {
                let Ok((attrs, verbs_count, props_count)) = get_object_info(tx, &player, &obj)
                else {
                    continue;
                };

                object_list.push((obj, attrs, verbs_count, props_count));
            }

            Ok(WorldStateResult::ObjectsList(object_list))
        }

        WorldStateAction::UpdateProperty {
            player,
            perms,
            obj,
            property,
            value,
        } => {
            let object = match_object_ref(&player, &perms, &obj, tx)
                .map_err(|_| CommandExecutionError(CommandError::NoObjectMatch))?;

            tx.update_property(&perms, &object, property, &value)
                .map_err(SchedulerError::PropertyRetrievalFailed)?;

            Ok(WorldStateResult::PropertyUpdated)
        }

        WorldStateAction::GetObjectFlags { obj } => {
            let flags = tx
                .flags_of(&obj)
                .map_err(SchedulerError::PropertyRetrievalFailed)?;

            Ok(WorldStateResult::ObjectFlags(flags.to_u16()))
        }
    }
}

fn get_object_info(
    tx: &mut dyn WorldState,
    player: &Obj,
    obj: &Obj,
) -> Result<(ObjAttrs, usize, usize), SchedulerError> {
    let owner = tx
        .owner_of(obj)
        .map_err(SchedulerError::PropertyRetrievalFailed)?;
    let parent = tx.parent_of(player, obj).unwrap_or(moor_var::NOTHING);
    let location = tx.location_of(player, obj).unwrap_or(moor_var::NOTHING);
    let flags = tx
        .flags_of(obj)
        .map_err(SchedulerError::PropertyRetrievalFailed)?;
    let name = tx.name_of(player, obj).unwrap_or_default();

    let attrs = ObjAttrs::new(owner, parent, location, flags, &name);

    let verbs = tx
        .verbs(player, obj)
        .map_err(SchedulerError::VerbRetrievalFailed)?;
    let verbs_count = verbs.len();

    let props = tx
        .properties(player, obj)
        .map_err(SchedulerError::PropertyRetrievalFailed)?;
    let props_count = props.len();
    Ok((attrs, verbs_count, props_count))
}

/// Match an ObjectRef to an actual Obj within a transaction.
/// This handles direct IDs, system object references, and name matching.
pub fn match_object_ref(
    player: &Obj,
    perms: &Obj,
    obj_ref: &ObjectRef,
    tx: &mut dyn WorldState,
) -> Result<Obj, WorldStateError> {
    match &obj_ref {
        ObjectRef::Id(obj) => {
            if !tx.valid(obj)? {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            }
            Ok(*obj)
        }
        ObjectRef::SysObj(names) => {
            // Follow the chain of properties from #0 to the actual object.
            // The final value has to be an object, or this is an error.
            let mut obj = SYSTEM_OBJECT;
            for name in names {
                let Ok(value) = tx.retrieve_property(perms, &obj, *name) else {
                    return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
                };
                let Some(o) = value.as_object() else {
                    return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
                };
                obj = o;
            }
            if !tx.valid(&obj)? {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            }
            Ok(obj)
        }
        ObjectRef::Match(object_name) => {
            let match_env = WsMatchEnv::new(tx, *perms);
            let matcher = ComplexObjectNameMatcher {
                env: match_env,
                player: *player,
                fuzzy_threshold: 0.5,
            };
            let Ok(match_result) = matcher.match_object(object_name) else {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            };
            let Some(o) = match_result.result else {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            };
            if !tx.valid(&o)? {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            }
            Ok(o)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::model::{ObjFlag, ObjectKind, WorldStateSource};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{NOTHING, SYSTEM_OBJECT, Symbol, v_int, v_str};

    fn setup_test_db() -> (TxDB, Box<dyn WorldState>) {
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let mut tx = db.new_world_state().unwrap();

        let sysobj = tx
            .create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                ObjFlag::all_flags(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        tx.update_property(
            &SYSTEM_OBJECT,
            &sysobj,
            Symbol::mk("name"),
            &v_str("system"),
        )
        .unwrap();
        tx.update_property(&SYSTEM_OBJECT, &sysobj, Symbol::mk("programmer"), &v_int(1))
            .unwrap();
        tx.update_property(&SYSTEM_OBJECT, &sysobj, Symbol::mk("wizard"), &v_int(1))
            .unwrap();

        (db, tx)
    }

    #[test]
    fn test_execute_empty_batch() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let results = execute_world_state_actions(&mut *tx, &config, vec![]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_execute_single_request_system_property() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let actions = vec![WorldStateAction::RequestSystemProperty {
            player: SYSTEM_OBJECT,
            obj: ObjectRef::Id(SYSTEM_OBJECT),
            property: Symbol::mk("name"),
        }];
        let results = execute_world_state_actions(&mut *tx, &config, actions).unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            WorldStateResult::SystemProperty(v) => assert_eq!(*v, v_str("system")),
            other => panic!("Expected SystemProperty, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_multiple_actions() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let actions = vec![
            WorldStateAction::RequestSystemProperty {
                player: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
                property: Symbol::mk("name"),
            },
            WorldStateAction::ResolveObject {
                player: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
            },
            WorldStateAction::GetObjectFlags { obj: SYSTEM_OBJECT },
        ];
        let results = execute_world_state_actions(&mut *tx, &config, actions).unwrap();
        assert_eq!(results.len(), 3);

        match &results[0] {
            WorldStateResult::SystemProperty(v) => assert_eq!(*v, v_str("system")),
            other => panic!("Expected SystemProperty, got {other:?}"),
        }
        match &results[1] {
            WorldStateResult::ResolvedObject(v) => {
                assert_eq!(*v, moor_var::v_obj(SYSTEM_OBJECT));
            }
            other => panic!("Expected ResolvedObject, got {other:?}"),
        }
        match &results[2] {
            WorldStateResult::ObjectFlags(flags) => {
                assert_ne!(*flags, 0); // Should have flags set
            }
            other => panic!("Expected ObjectFlags, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_request_properties() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let actions = vec![WorldStateAction::RequestProperties {
            player: SYSTEM_OBJECT,
            perms: SYSTEM_OBJECT,
            obj: ObjectRef::Id(SYSTEM_OBJECT),
            inherited: false,
        }];
        let results = execute_world_state_actions(&mut *tx, &config, actions).unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            WorldStateResult::Properties(_props) => {
                // Successfully retrieved property list (may be empty for built-in system props)
            }
            other => panic!("Expected Properties, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_request_all_objects() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let actions = vec![WorldStateAction::RequestAllObjects {
            player: SYSTEM_OBJECT,
        }];
        let results = execute_world_state_actions(&mut *tx, &config, actions).unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            WorldStateResult::AllObjects(objs) => {
                assert!(objs.contains(&SYSTEM_OBJECT));
            }
            other => panic!("Expected AllObjects, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_update_property() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let actions = vec![
            WorldStateAction::UpdateProperty {
                player: SYSTEM_OBJECT,
                perms: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
                property: Symbol::mk("name"),
                value: v_str("new_name"),
            },
            WorldStateAction::RequestSystemProperty {
                player: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
                property: Symbol::mk("name"),
            },
        ];
        let results = execute_world_state_actions(&mut *tx, &config, actions).unwrap();
        assert_eq!(results.len(), 2);
        match &results[0] {
            WorldStateResult::PropertyUpdated => {}
            other => panic!("Expected PropertyUpdated, got {other:?}"),
        }
        match &results[1] {
            WorldStateResult::SystemProperty(v) => assert_eq!(*v, v_str("new_name")),
            other => panic!("Expected SystemProperty, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_list_objects() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        let actions = vec![WorldStateAction::ListObjects {
            player: SYSTEM_OBJECT,
        }];
        let results = execute_world_state_actions(&mut *tx, &config, actions).unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            WorldStateResult::ObjectsList(objects) => {
                assert!(!objects.is_empty());
                // Should contain the system object
                assert!(objects.iter().any(|(obj, _, _, _)| *obj == SYSTEM_OBJECT));
            }
            other => panic!("Expected ObjectsList, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_error_propagation() {
        let (_db, mut tx) = setup_test_db();
        let config = Config::default();
        // Try to resolve a non-existent object
        let actions = vec![WorldStateAction::ResolveObject {
            player: SYSTEM_OBJECT,
            obj: ObjectRef::Match("nonexistent_object_xyz".to_string()),
        }];
        // This should return a ResolvedObject with v_err or fail, depending on implementation
        let results = execute_world_state_actions(&mut *tx, &config, actions);
        // The resolve action returns an error var rather than failing the batch
        match results {
            Ok(results) => {
                assert_eq!(results.len(), 1);
                match &results[0] {
                    WorldStateResult::ResolvedObject(v) => {
                        // Should be an error value for not found
                        assert!(
                            matches!(v.variant(), moor_var::Variant::Err(_)),
                            "Expected error variant, got {v:?}"
                        );
                    }
                    other => panic!("Expected ResolvedObject, got {other:?}"),
                }
            }
            Err(_) => {
                // Also acceptable — the batch may fail on bad object match
            }
        }
    }
}

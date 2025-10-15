// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{
    config::Config,
    tasks::world_state_action::{WorldStateAction, WorldStateResult},
};

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
                Ok(CommitResult::ConflictRetry) => {
                    return Err(CommandExecutionError(CommandError::DatabaseError(
                        WorldStateError::DatabaseError("Transaction conflict".to_string()),
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

                let (_, verbdef) = self
                    .tx
                    .find_method_verb_on(&perms, &object, verb_name)
                    .map_err(|_| VerbProgramFailed(VerbProgramError::NoVerbToProgram))?;

                if verbdef.location() != object {
                    return Err(VerbProgramFailed(VerbProgramError::NoVerbToProgram));
                }

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
                    .map_err(|_| VerbProgramFailed(VerbProgramError::NoVerbToProgram))?;

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
                        let ancestor_properties = self
                            .tx
                            .properties(&perms, &ancestor)
                            .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

                        for prop in ancestor_properties.iter() {
                            let (info, prop_perms) = self
                                .tx
                                .get_property_info(&perms, &ancestor, prop.name())
                                .map_err(|e| {
                                    CommandExecutionError(CommandError::DatabaseError(e))
                                })?;
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
                        let ancestor_verbs = self
                            .tx
                            .verbs(&perms, &ancestor)
                            .map_err(SchedulerError::VerbRetrievalFailed)?;
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

                let (program, verbdef) = self
                    .tx
                    .find_method_verb_on(&perms, &object, verb)
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
        }
    }
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
            };
            let Ok(Some(o)) = matcher.match_object(object_name) else {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            };
            if !tx.valid(&o)? {
                return Err(WorldStateError::ObjectNotFound(obj_ref.clone()));
            }
            Ok(o)
        }
    }
}

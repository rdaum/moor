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

//! Command suggestion engine for autocompletion and object-first interactions

use bincode::{Decode, Encode};
use lazy_static::lazy_static;
use moor_common::model::{ArgSpec, Named, PrepSpec, ValSet, VerbFlag, WorldState};
use moor_common::tasks::SchedulerError;
use moor_var::{Obj, Symbol};

lazy_static! {
    // Common verb symbols for efficient comparison and priority ordering
    static ref LOOK: Symbol = Symbol::mk("look");
    static ref L: Symbol = Symbol::mk("l");
    static ref EXAMINE: Symbol = Symbol::mk("examine");
    static ref TAKE: Symbol = Symbol::mk("take");
    static ref GET: Symbol = Symbol::mk("get");
    static ref DROP: Symbol = Symbol::mk("drop");
    static ref OPEN: Symbol = Symbol::mk("open");
    static ref CLOSE: Symbol = Symbol::mk("close");
    static ref USE: Symbol = Symbol::mk("use");
    static ref TALK: Symbol = Symbol::mk("talk");
    static ref SAY: Symbol = Symbol::mk("say");
    static ref GIVE: Symbol = Symbol::mk("give");
    static ref PUT: Symbol = Symbol::mk("put");
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum SuggestionMode {
    /// Show actions available on selected object: obj#123 -> ["look lamp", "take lamp", "light lamp"]
    ObjectActions,
    /// Show all available actions in player's environment (player, room, inventory, room contents, other players)
    EnvironmentActions,
    /// Show objects that work with a verb: "put" -> ["lamp", "book", "key"]
    VerbTargets(String),
    /// Show objects that work as indirect objects: "put lamp" -> ["in box", "on table", "under bed"]
    IndirectTargets(String, Option<Obj>), // verb, direct_object
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct CommandSuggestionsResponse {
    /// Complete command suggestions ready to execute
    pub action_suggestions: Vec<ActionSuggestion>,
    /// Standalone verb suggestions (for command completion)
    pub verb_suggestions: Vec<VerbSuggestion>,
    /// Object suggestions (for object selection)
    pub object_suggestions: Vec<ObjectSuggestion>,
    /// The context that was used for these suggestions
    pub suggestion_context: SuggestionContext,
    /// Whether more suggestions are available
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct ActionSuggestion {
    /// The verb being suggested
    pub verb: Symbol,
    /// Direct object (if any)
    pub dobj: Option<Obj>,
    /// Direct object name for display
    pub dobjstr: Option<String>,
    /// Preposition (if any)
    pub prep: PrepSpec,
    /// Preposition string for display
    pub prepstr: Option<String>,
    /// Indirect object (if any)
    pub iobj: Option<Obj>,
    /// Indirect object name for display
    pub iobjstr: Option<String>,
    /// Whether this action requires additional input from user
    pub needs_input: bool,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct VerbSuggestion {
    /// The verb name (e.g. "look", "take", "put")
    pub verb_name: Symbol,
    /// Object the verb is defined on
    pub object: Obj,
    /// Display name of the object
    pub object_name: String,
    /// Complete command suggestion (e.g. "look lamp")
    pub full_command: String,
    /// Verb argument specification (dobj, prep, iobj)
    pub args_spec: Vec<Symbol>,
    /// Optional help text or description
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct ObjectSuggestion {
    /// The object reference
    pub object: Obj,
    /// Primary name of the object
    pub name: String,
    /// Alternative names/aliases
    pub aliases: Vec<String>,
    /// Type classification ("player", "room", "thing", etc.)
    pub object_type: String,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum SuggestionContext {
    /// Empty command or partial verb
    Verb,
    /// After verb, looking for direct object
    DirectObject(String),
    /// After direct object, looking for preposition
    Preposition(String),
    /// After preposition, looking for indirect object
    IndirectObject(String),
    /// Object-first mode: show actions available on specific object
    ObjectActions(Obj),
    /// Environment mode: show all available actions in player's environment
    Environment,
}

impl Default for CommandSuggestionsResponse {
    fn default() -> Self {
        Self {
            action_suggestions: Vec::new(),
            verb_suggestions: Vec::new(),
            object_suggestions: Vec::new(),
            suggestion_context: SuggestionContext::Verb,
            has_more: false,
        }
    }
}

/// Core suggestion engine that provides command autocompletion functionality
pub struct CommandSuggestionEngine;

impl CommandSuggestionEngine {
    /// Get command suggestions based on the request parameters
    pub fn get_suggestions(
        player: &Obj,
        selected_object: Option<&Obj>,
        suggestion_mode: SuggestionMode,
        max_suggestions: usize,
        world_state: &mut dyn WorldState,
    ) -> Result<CommandSuggestionsResponse, SchedulerError> {
        match suggestion_mode {
            SuggestionMode::ObjectActions => {
                let obj = selected_object.ok_or(SchedulerError::CommandExecutionError(
                    moor_common::tasks::CommandError::CouldNotParseCommand,
                ))?;
                Self::suggest_object_actions(player, obj, max_suggestions, world_state)
            }
            SuggestionMode::EnvironmentActions => {
                Self::suggest_environment_actions(player, max_suggestions, world_state)
            }
            SuggestionMode::VerbTargets(verb) => {
                Self::suggest_verb_targets(player, &verb, max_suggestions, world_state)
            }
            SuggestionMode::IndirectTargets(verb, dobj) => Self::suggest_indirect_targets(
                player,
                &verb,
                dobj.as_ref(),
                max_suggestions,
                world_state,
            ),
        }
    }

    /// Suggest actions available on a specific object
    fn suggest_object_actions(
        player: &Obj,
        target_object: &Obj,
        max_suggestions: usize,
        world_state: &mut dyn WorldState,
    ) -> Result<CommandSuggestionsResponse, SchedulerError> {
        let mut action_suggestions = Vec::new();

        // Get object names for display
        let (primary_name, _aliases) = world_state
            .names_of(player, target_object)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        // Get command verbs available on the target object (filters out "this none this" verbs)
        let verbs = world_state
            .command_verbs_on(player, target_object)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        for verb_def in verbs.iter() {
            // Check if player can see this verb
            if !verb_def.flags().contains(VerbFlag::Read) {
                continue;
            }

            let args_spec = verb_def.args();

            // Skip method verbs with "this none this" pattern - these are not user commands
            if matches!(args_spec.dobj, ArgSpec::This)
                && matches!(args_spec.prep, PrepSpec::None)
                && matches!(args_spec.iobj, ArgSpec::This)
            {
                continue;
            }

            for verb_name in verb_def.names() {
                // Determine if this action needs additional input
                let needs_input = matches!(args_spec.iobj, ArgSpec::This | ArgSpec::Any)
                    || matches!(args_spec.prep, PrepSpec::Any | PrepSpec::Other(_));

                // For object actions, the target object is typically the direct object
                let (dobj, dobjstr) = if matches!(args_spec.dobj, ArgSpec::This | ArgSpec::Any) {
                    (Some(*target_object), Some(primary_name.clone()))
                } else {
                    (None, None)
                };

                action_suggestions.push(ActionSuggestion {
                    verb: *verb_name,
                    dobj,
                    dobjstr,
                    prep: args_spec.prep,
                    prepstr: None, // TODO: Could populate if prep is Other(preposition)
                    iobj: None,    // Object actions don't specify indirect objects
                    iobjstr: None,
                    needs_input,
                });

                // Don't exceed the limit
                if action_suggestions.len() >= max_suggestions {
                    break;
                }
            }

            if action_suggestions.len() >= max_suggestions {
                break;
            }
        }

        // Sort by verb priority (common verbs first)
        action_suggestions.sort_by_key(|a| Self::get_verb_priority(&a.verb));
        action_suggestions.truncate(max_suggestions);

        Ok(CommandSuggestionsResponse {
            action_suggestions,
            verb_suggestions: Vec::new(),
            object_suggestions: Vec::new(),
            suggestion_context: SuggestionContext::ObjectActions(*target_object),
            has_more: false,
        })
    }

    /// Suggest all available actions in the player's environment
    fn suggest_environment_actions(
        player: &Obj,
        max_suggestions: usize,
        world_state: &mut dyn WorldState,
    ) -> Result<CommandSuggestionsResponse, SchedulerError> {
        let mut action_suggestions = Vec::new();
        let mut seen_verb_names = std::collections::HashSet::new();

        // Get player's current location
        let player_location = world_state
            .location_of(player, player)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        // 1. Collect verbs from the player themselves
        Self::collect_verbs_from_object(
            player,
            player,
            world_state,
            &mut action_suggestions,
            &mut seen_verb_names,
            "self",
        )?;

        // 2. Collect verbs from the current room (if not #-1)
        if !player_location.is_nothing() {
            Self::collect_verbs_from_object(
                player,
                &player_location,
                world_state,
                &mut action_suggestions,
                &mut seen_verb_names,
                "room",
            )?;

            // 3. Collect verbs from objects in the room
            let room_contents = world_state
                .contents_of(player, &player_location)
                .map_err(|_| SchedulerError::CouldNotStartTask)?;

            for obj in room_contents.iter() {
                if obj == *player {
                    continue; // Skip the player themselves
                }

                // Get object name for display
                let obj_name = world_state
                    .name_of(player, &obj)
                    .unwrap_or_else(|_| format!("#{}", obj.id()));

                Self::collect_verbs_from_object(
                    player,
                    &obj,
                    world_state,
                    &mut action_suggestions,
                    &mut seen_verb_names,
                    &obj_name,
                )?;
            }
        }

        // 4. Collect verbs from objects in player's inventory
        let inventory = world_state
            .contents_of(player, player)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        for obj in inventory.iter() {
            // Get object name for display
            let obj_name = world_state
                .name_of(player, &obj)
                .unwrap_or_else(|_| format!("#{}", obj.id()));

            Self::collect_verbs_from_object(
                player,
                &obj,
                world_state,
                &mut action_suggestions,
                &mut seen_verb_names,
                &obj_name,
            )?;
        }

        // Sort by verb priority and limit results
        action_suggestions.sort_by_key(|a| Self::get_verb_priority(&a.verb));
        action_suggestions.truncate(max_suggestions);

        Ok(CommandSuggestionsResponse {
            action_suggestions,
            verb_suggestions: Vec::new(),
            object_suggestions: Vec::new(),
            suggestion_context: SuggestionContext::Environment,
            has_more: false, // TODO: Could track if we hit the limit
        })
    }

    /// Helper method to collect verbs from a specific object
    fn collect_verbs_from_object(
        player: &Obj,
        target_object: &Obj,
        world_state: &mut dyn WorldState,
        action_suggestions: &mut Vec<ActionSuggestion>,
        seen_verb_names: &mut std::collections::HashSet<Symbol>,
        object_display_name: &str,
    ) -> Result<(), SchedulerError> {
        // Get command verbs available on the target object
        let verbs = world_state
            .command_verbs_on(player, target_object)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        for verb_def in verbs.iter() {
            // Check if player can see this verb
            if !verb_def.flags().contains(VerbFlag::Read) {
                continue;
            }

            let args_spec = verb_def.args();

            for verb_name in verb_def.names() {
                // Skip if we've already seen this verb name (avoid duplicates)
                if seen_verb_names.contains(verb_name) {
                    continue;
                }

                seen_verb_names.insert(*verb_name);

                // Determine if this action needs additional input
                let needs_input = matches!(args_spec.iobj, ArgSpec::This | ArgSpec::Any)
                    || matches!(args_spec.prep, PrepSpec::Any | PrepSpec::Other(_));

                // Determine the direct object based on the verb's arg spec
                let (dobj, dobjstr) = match args_spec.dobj {
                    ArgSpec::This => (Some(*target_object), Some(object_display_name.to_string())),
                    ArgSpec::Any => {
                        // For "any" verbs, we could suggest the object but it's optional
                        (Some(*target_object), Some(object_display_name.to_string()))
                    }
                    ArgSpec::None => (None, None),
                };

                action_suggestions.push(ActionSuggestion {
                    verb: *verb_name,
                    dobj,
                    dobjstr,
                    prep: args_spec.prep,
                    prepstr: None, // TODO: Could populate if prep is Other(preposition)
                    iobj: None,    // Environment actions don't specify indirect objects yet
                    iobjstr: None,
                    needs_input,
                });
            }
        }

        Ok(())
    }

    /// Suggest objects that work with a specific verb
    fn suggest_verb_targets(
        _player: &Obj,
        _verb: &str,
        _max_suggestions: usize,
        _world_state: &mut dyn WorldState,
    ) -> Result<CommandSuggestionsResponse, SchedulerError> {
        // TODO: Implement verb target suggestions
        Ok(CommandSuggestionsResponse::default())
    }

    /// Suggest indirect objects for verb + direct object combinations
    fn suggest_indirect_targets(
        _player: &Obj,
        _verb: &str,
        _direct_object: Option<&Obj>,
        _max_suggestions: usize,
        _world_state: &mut dyn WorldState,
    ) -> Result<CommandSuggestionsResponse, SchedulerError> {
        // TODO: Implement indirect target suggestions
        Ok(CommandSuggestionsResponse::default())
    }

    /// Get priority for verb ordering (lower numbers = higher priority)
    ///
    /// TODO: This hardcoded priority system should be made configurable or based on:
    /// - Actual usage patterns/frequency
    /// - Verb flags or properties
    /// - MOO world-specific configuration
    /// - Learning from player behavior
    fn get_verb_priority(verb: &Symbol) -> u32 {
        if *verb == *LOOK || *verb == *EXAMINE || *verb == *L {
            1
        } else if *verb == *TAKE || *verb == *GET {
            2
        } else if *verb == *USE {
            3
        } else if *verb == *OPEN {
            4
        } else if *verb == *CLOSE {
            5
        } else if *verb == *TALK || *verb == *SAY {
            6
        } else if *verb == *GIVE {
            7
        } else if *verb == *PUT {
            8
        } else if *verb == *DROP {
            9
        } else {
            100 // Everything else gets lower priority
        }
    }
}

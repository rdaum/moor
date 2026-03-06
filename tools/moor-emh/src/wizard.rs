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

use eyre::{Report, bail, eyre};
use moor_common::model::{ObjFlag, ValSet};
use moor_db::Database;
use moor_var::Obj;
use tracing::{info, warn};

/// Find a wizard object to use for admin commands
pub(crate) fn find_wizard(
    database: &dyn Database,
    requested_wizard: Option<&str>,
) -> Result<Obj, Report> {
    let tx = database.new_world_state()?;

    if let Some(wiz_ref) = requested_wizard {
        let wizard = Obj::try_from(wiz_ref)
            .map_err(|e| eyre!("Invalid wizard object reference '{wiz_ref}': {e}"))?;
        if tx.valid(&wizard)? && tx.flags_of(&wizard)?.contains(ObjFlag::Wizard) {
            return Ok(wizard);
        }
        warn!(
            "Requested wizard {} is not valid or not a wizard",
            wizard.to_literal()
        );
    }

    // Find all wizard objects and choose deterministically.
    let all_objects = tx.all_objects()?;
    info!("Scanning {} objects for wizard", all_objects.len());

    let mut wizard_candidates = Vec::new();
    for obj in all_objects.iter() {
        if tx.flags_of(&obj)?.contains(ObjFlag::Wizard) {
            wizard_candidates.push(obj);
        }
    }

    if wizard_candidates.is_empty() {
        bail!("No wizard objects found in database");
    }

    wizard_candidates.sort_unstable();
    let wizard = wizard_candidates[0];
    if wizard_candidates.len() > 1 {
        warn!(
            "Multiple wizard objects found ({}); selecting deterministic wizard {}",
            wizard_candidates.len(),
            wizard.to_literal()
        );
    } else {
        info!("Using wizard object: {}", wizard.to_literal());
    }

    Ok(wizard)
}

// SPDX-License-Identifier: GPL-3.0-only

//! The pure dispatch: look the action up, apply the §15 gate, then map the target to an effect.

use super::effect::{DispatchError, Effect};
use super::enable::{GateState, enabled};
use super::id::ActionId;
use super::spec::{Target, spec};

/// Dispatch the action in the gate state.
///
/// # Errors
///
/// Returns `UnknownAction` when the id is not in the action table, and `Disabled` when the §15
/// gate refuses the action in this state.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "the frozen registry signature takes the gate state by reference"
)]
pub(crate) fn dispatch(id: ActionId, gate: &GateState) -> Result<Effect, DispatchError> {
    let action = spec(id).ok_or(DispatchError::UnknownAction(id))?;
    if !enabled(id, gate) {
        return Err(DispatchError::Disabled(id));
    }
    Ok(effect(action.target))
}

/// Map a target to its effect.
const fn effect(target: Target) -> Effect {
    match target {
        Target::Window(window) => Effect::OpenWindow(window),
        Target::Command(command) => Effect::RunCommand(command),
        Target::Check(check) => Effect::ToggleCheck(check),
        Target::Density(spelling) => Effect::SetDensity(spelling),
        Target::TextSize(spelling) => Effect::SetTextSize(spelling),
        Target::EditRide => Effect::OpenEditRide,
        Target::Native { dialog, then } => Effect::OpenNative { dialog, then },
    }
}

#[cfg(test)]
mod tests;

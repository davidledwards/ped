//! A formal interface for interacting with arbitrary editing operations.

use crate::env::Environment;
use crate::user::Question;

/// A function type that implements an editing operation.
pub type Operation = fn(&mut Environment) -> Option<Action>;

/// An action returned by an [`OpFn`] that is meant to be carried out by a controller
/// orchestrating calls to such functions.
pub enum Action {
    Quit,
    Redraw,
    Echo(String),
    Question(Box<dyn Question>),
}

impl Action {
    pub fn quit() -> Option<Action> {
        Some(Action::Quit)
    }

    pub fn redraw() -> Option<Action> {
        Some(Action::Redraw)
    }

    pub fn echo<T: ToString + ?Sized>(text: &T) -> Option<Action> {
        let action = Action::Echo(text.to_string());
        Some(action)
    }

    pub fn question(question: Box<dyn Question>) -> Option<Action> {
        let action = Action::Question(question);
        Some(action)
    }
}

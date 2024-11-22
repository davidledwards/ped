//! Input completion types.
use std::str::FromStr;

pub type CompleterFn = dyn Fn(CompleterEvent) -> Option<CompleterAction>;

pub enum CompleterEvent {
    Initialize,
    Edit(String),
    Suggest(String),
    Finalize(String),
}

pub enum CompleterAction {
    Hint(String),
    Replace(String, Option<String>),
    Accept(String),
}

impl CompleterAction {
    pub fn as_hint(hint: &str) -> Option<CompleterAction> {
        Some(CompleterAction::Hint(hint.to_string()))
    }

    pub fn as_replace(input: &str, hint: Option<&str>) -> Option<CompleterAction> {
        Some(CompleterAction::Replace(
            input.to_string(),
            hint.map(|s| s.to_string()),
        ))
    }

    pub fn as_accept(input: String) -> Option<CompleterAction> {
        Some(CompleterAction::Accept(input))
    }
}

pub fn yes_no(event: CompleterEvent) -> Option<CompleterAction> {
    const HINT: &str = " (y)es, (n)o";
    const ACCEPTED: [&str; 4] = ["y", "n", "Y", "N"];
    basic_completer(event, &ACCEPTED, HINT)
}

pub fn yes_no_all(event: CompleterEvent) -> Option<CompleterAction> {
    const HINT: &str = " (y)es, (n)o, (a)ll";
    const ACCEPTED: [&str; 6] = ["y", "n", "a", "Y", "N", "A"];
    basic_completer(event, &ACCEPTED, HINT)
}

fn basic_completer(
    event: CompleterEvent,
    accepted: &[&str],
    hint: &str,
) -> Option<CompleterAction> {
    match event {
        CompleterEvent::Initialize => CompleterAction::as_hint(hint),
        CompleterEvent::Edit(input) => match input.as_str() {
            input if accepted.contains(&input) => None,
            "" => None,
            _ => CompleterAction::as_hint(hint),
        },
        CompleterEvent::Suggest(_) => CompleterAction::as_hint(hint),
        CompleterEvent::Finalize(input) => match input.as_str() {
            input if accepted.contains(&input) => CompleterAction::as_accept(input.to_lowercase()),
            _ => CompleterAction::as_hint(hint),
        },
    }
}

pub fn number<T: FromStr>(event: CompleterEvent) -> Option<CompleterAction> {
    const HINT: &str = " (not valid)";

    fn validate<T: FromStr>(input: &str) -> Option<&str> {
        let input = input.trim();
        if input.len() > 0 {
            input.parse::<T>().map(|_| input).ok()
        } else {
            Some(input)
        }
    }

    match event {
        CompleterEvent::Initialize => None,
        CompleterEvent::Edit(input) => {
            if let Some(_) = validate::<T>(&input) {
                None
            } else {
                CompleterAction::as_hint(HINT)
            }
        }
        CompleterEvent::Suggest(_) => None,
        CompleterEvent::Finalize(input) => {
            if let Some(input) = validate::<T>(&input) {
                CompleterAction::as_accept(input.to_string())
            } else {
                CompleterAction::as_hint(HINT)
            }
        }
    }
}

pub fn file(event: CompleterEvent) -> Option<CompleterAction> {
    match event {
        CompleterEvent::Initialize => None,
        CompleterEvent::Edit(_) => None,
        CompleterEvent::Suggest(_) => None,
        CompleterEvent::Finalize(_) => None,
    }
}

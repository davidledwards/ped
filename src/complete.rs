//! Input completion types.

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

    match event {
        CompleterEvent::Initialize => CompleterAction::as_hint(HINT),
        CompleterEvent::Edit(input) => match input.as_str() {
            input if ACCEPTED.contains(&input) => None,
            "" => None,
            _ => CompleterAction::as_hint(HINT),
        },
        CompleterEvent::Suggest(_) => CompleterAction::as_hint(HINT),
        CompleterEvent::Finalize(input) => match input.as_str() {
            input if ACCEPTED.contains(&input) => CompleterAction::as_accept(input.to_lowercase()),
            _ => CompleterAction::as_hint(HINT),
        },
    }
}

pub fn yes_no_all(event: CompleterEvent) -> Option<CompleterAction> {
    const HINT: &str = " (y)es, (n)o, (a)ll";
    const ACCEPTED: [&str; 6] = ["y", "n", "a", "Y", "N", "A"];

    match event {
        CompleterEvent::Initialize => CompleterAction::as_hint(HINT),
        CompleterEvent::Edit(input) => match input.as_str() {
            input if ACCEPTED.contains(&input) => None,
            "" => None,
            _ => CompleterAction::as_hint(HINT),
        },
        CompleterEvent::Suggest(_) => CompleterAction::as_hint(HINT),
        CompleterEvent::Finalize(input) => match input.as_str() {
            input if ACCEPTED.contains(&input) => CompleterAction::as_accept(input.to_lowercase()),
            _ => CompleterAction::as_hint(HINT),
        },
    }
}

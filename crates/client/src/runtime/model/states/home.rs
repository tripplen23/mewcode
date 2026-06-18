use crate::net::SessionSummary;

/// State backing [`super::Screen::Home`].
#[derive(Debug)]
pub struct HomeState {
    /// Sessions shown in the list.
    pub sessions: Vec<SessionSummary>,
    /// Index of the highlighted row.
    pub selected: usize,
    /// `true` while the session list is being fetched.
    pub loading: bool,
}

impl HomeState {
    /// A Home screen in its initial loading state, before sessions arrive.
    pub fn loading() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
            loading: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSession {
    pub id: u64,
    pub target_token: String,
    pub draft: String,
    pub released: bool,
}

#[derive(Debug, Default)]
pub struct VoiceInputState {
    target_token: Option<String>,
    session: Option<VoiceSession>,
    next_session_id: u64,
}

impl VoiceInputState {
    pub fn set_target(&mut self, target_token: Option<String>) {
        self.target_token = target_token;
    }

    pub fn begin(&mut self) -> Option<VoiceSession> {
        if self.session.is_some() {
            return None;
        }
        let target_token = self.target_token.clone()?;
        self.next_session_id = self.next_session_id.saturating_add(1).max(1);
        let session = VoiceSession {
            id: self.next_session_id,
            target_token,
            draft: String::new(),
            released: false,
        };
        self.session = Some(session.clone());
        Some(session)
    }

    pub fn record_draft(&mut self, session_id: u64, draft: String) -> bool {
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        if session.id != session_id {
            return false;
        }
        if session.draft == draft {
            return false;
        }
        session.draft = draft;
        true
    }

    pub fn release(&mut self) -> Option<VoiceSession> {
        let session = self.session.as_mut()?;
        if session.released {
            return None;
        }
        session.released = true;
        Some(session.clone())
    }

    pub fn finish(&mut self, session_id: u64) -> bool {
        if self.session.as_ref().map(|session| session.id) != Some(session_id) {
            return false;
        }
        self.session = None;
        true
    }

    pub fn cancel_current(&mut self) -> Option<VoiceSession> {
        self.session.take()
    }

    pub fn current(&self) -> Option<&VoiceSession> {
        self.session.as_ref()
    }

    pub fn has_target(&self) -> bool {
        self.target_token.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::VoiceInputState;

    #[test]
    fn ignores_fn_without_an_editable_target() {
        assert!(VoiceInputState::default().begin().is_none());
    }

    #[test]
    fn reports_whether_a_standard_text_target_is_available() {
        let mut state = VoiceInputState::default();
        assert!(!state.has_target());
        state.set_target(Some("content".to_owned()));
        assert!(state.has_target());
        state.set_target(None);
        assert!(!state.has_target());
    }

    #[test]
    fn captures_the_target_for_the_whole_session() {
        let mut state = VoiceInputState::default();
        state.set_target(Some("title".to_owned()));
        let session = state.begin().unwrap();
        state.set_target(Some("note".to_owned()));

        assert_eq!(session.target_token, "title");
        assert_eq!(state.current().unwrap().target_token, "title");
    }

    #[test]
    fn treats_native_text_as_a_complete_replacement_draft() {
        let mut state = VoiceInputState::default();
        state.set_target(Some("content".to_owned()));
        let session = state.begin().unwrap();

        assert!(state.record_draft(session.id, "我想研究".to_owned()));
        assert!(!state.record_draft(session.id, "我想研究".to_owned()));
        assert!(state.record_draft(session.id, "我想研究一下。".to_owned()));
        assert_eq!(state.current().unwrap().draft, "我想研究一下。");
    }

    #[test]
    fn release_keeps_the_session_open_for_final_corrections() {
        let mut state = VoiceInputState::default();
        state.set_target(Some("content".to_owned()));
        let session = state.begin().unwrap();

        assert!(state.release().unwrap().released);
        assert!(state.release().is_none());
        assert!(state.record_draft(session.id, "松开后修订。".to_owned()));
        assert_eq!(state.current().unwrap().draft, "松开后修订。");
    }

    #[test]
    fn stale_finish_and_draft_cannot_touch_a_newer_session() {
        let mut state = VoiceInputState::default();
        state.set_target(Some("content".to_owned()));
        let first = state.begin().unwrap();
        assert!(state.finish(first.id));
        let second = state.begin().unwrap();

        assert!(!state.record_draft(first.id, "旧结果".to_owned()));
        assert!(!state.finish(first.id));
        assert_eq!(state.current().unwrap().id, second.id);
    }

    #[test]
    fn function_combo_can_cancel_the_current_session() {
        let mut state = VoiceInputState::default();
        state.set_target(Some("content".to_owned()));
        let session = state.begin().unwrap();

        assert_eq!(state.cancel_current().unwrap().id, session.id);
        assert!(state.current().is_none());
    }
}

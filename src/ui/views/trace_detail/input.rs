//! Input handling for the Trace Detail View

use crossterm::event::KeyEvent;

use crate::keys;

use super::{TraceDetailAction, TraceDetailView};

impl TraceDetailView {
    /// Handle a key event, returning the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> TraceDetailAction {
        let code = key.code;
        if keys::is_move_down(code) {
            self.select_next();
            return TraceDetailAction::None;
        }
        if keys::is_move_up(code) {
            self.select_prev();
            return TraceDetailAction::None;
        }
        match code {
            keys::YANK => match self.current_url() {
                Some(url) => TraceDetailAction::CopyUrl(url.to_string()),
                None => TraceDetailAction::NoUrl,
            },
            keys::QUIT | keys::ESC => TraceDetailAction::Back,
            _ => TraceDetailAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{
        ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRecord,
    };
    use crossterm::event::KeyCode;

    fn key(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }

    fn record_with_url(url: Option<&str>) -> TraceRecord {
        TraceRecord {
            timestamp: "t".to_string(),
            vcs: None,
            tool_name: Some("x".to_string()),
            tool_version: None,
            files: vec![TraceFile {
                path: "a.rs".to_string(),
                conversations: vec![TraceConversation {
                    url: url.map(str::to_string),
                    contributor: Some(TraceContributor {
                        kind: ContributorKind::Ai,
                        model_id: None,
                    }),
                    ranges: vec![],
                    related: vec![],
                }],
            }],
        }
    }

    /// Move the cursor down with `j` until it sits on a URL row.
    fn cursor_to_first_url(v: &mut TraceDetailView) {
        while v.current_url().is_none() {
            let before = v.handle_key(key('j'));
            assert_eq!(before, TraceDetailAction::None);
            if v.current_url().is_some() {
                break;
            }
        }
    }

    #[test]
    fn yank_on_url_row_copies_it() {
        let mut v = TraceDetailView::new("x".to_string(), vec![record_with_url(Some("u1"))]);
        cursor_to_first_url(&mut v);
        assert_eq!(
            v.handle_key(key('y')),
            TraceDetailAction::CopyUrl("u1".to_string())
        );
    }

    #[test]
    fn yank_on_non_url_row_returns_no_url() {
        // cursor starts on the header row (not a URL)
        let mut v = TraceDetailView::new("x".to_string(), vec![record_with_url(Some("u1"))]);
        assert!(v.current_url().is_none());
        assert_eq!(v.handle_key(key('y')), TraceDetailAction::NoUrl);
    }

    #[test]
    fn quit_returns_back() {
        let mut v = TraceDetailView::new("x".to_string(), vec![record_with_url(Some("u1"))]);
        assert_eq!(v.handle_key(key('q')), TraceDetailAction::Back);
    }

    #[test]
    fn jk_scrolls_over_all_rows() {
        let mut r = record_with_url(Some("u1"));
        r.files[0].conversations[0].related = vec![crate::trace::TraceRelated {
            rel_type: "pr".to_string(),
            url: "u2".to_string(),
        }];
        let mut v = TraceDetailView::new("x".to_string(), vec![r]);
        // j reaches both URL rows (u1 then u2) as it scrolls down
        cursor_to_first_url(&mut v);
        assert_eq!(v.current_url(), Some("u1"));
        v.handle_key(key('j'));
        assert_eq!(v.current_url(), Some("u2"));
        v.handle_key(key('k'));
        assert_eq!(v.current_url(), Some("u1"));
    }
}

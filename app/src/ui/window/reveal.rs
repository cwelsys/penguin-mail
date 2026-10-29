//! Waiting for a body before a reply quotes it.
//!
//! Opening a thread takes a store read and then a trip to Gmail for the
//! bodies. A reply waits for that, so it can quote the body rather than
//! the snippet, but no longer than [`REVEAL_WAIT`]. By then the reader
//! may have clicked another conversation, and a reply to that one is a
//! reply nobody asked for, so the wait holds the target it started on
//! and gives up once that target leaves the screen. [`body_settles`] is
//! the wait the Reply button makes; [`reply_when_open`] is the one a
//! reply asked for from outside the window makes, such as the Reply
//! button on a new-mail notification, which first waits for the thread
//! to open at all.

use std::time::Duration;

use mailrs_domain::{AccountId, Target};

use crate::wanted::{Answer, Screen, Wanted};

/// How long the reply waits for the thread and its body to arrive before
/// it quotes the snippet instead.
pub(super) const REVEAL_WAIT: Duration = Duration::from_secs(5);

/// How often that wait looks at the conversation.
pub(super) const REVEAL_STEP: Duration = Duration::from_millis(100);

/// The window as the wait sees it.
pub(super) trait Waiting: Screen {
    /// The conversation on screen, if any.
    fn target(&self) -> Option<Target>;
    /// Whether the conversation on screen holds the body `message` would
    /// quote, or its newest message's when `message` names none. A body
    /// that failed for good counts: nothing more will come.
    fn quotable(&self, message: Option<&str>) -> bool;
    /// Waits `step`.
    fn sleep(&self, step: Duration) -> Answer<'_, ()>;
    /// Opens a reply to the newest message on screen.
    fn reply(&self);
    /// Says something at the bottom of the window.
    fn toast(&self, text: String);
}

/// Replies to the thread `thread_id` of `account_id` once it is on screen,
/// with its body when that arrives within `wait`. The thread may open as
/// one message of it, when the list shows messages, so the target is
/// taken from the screen the first time the thread shows up there.
pub(super) async fn reply_when_open(
    window: &dyn Waiting,
    account_id: AccountId,
    thread_id: &str,
    wait: Duration,
) {
    let steps = steps_in(wait);
    let mut held: Option<Target> = None;
    for step in 0..=steps {
        if held.is_none() {
            held = window
                .target()
                .filter(|t| t.account_id == account_id && t.thread_id == thread_id);
        }
        if let Some(target) = &held {
            // The reader opened something else while the thread loaded.
            if !window.is_showing(target) {
                return;
            }
            if window.quotable(None) {
                break;
            }
        }
        if step < steps {
            window.sleep(REVEAL_STEP).await;
        }
    }
    match held {
        Some(target) => {
            Wanted::new(window, window, target).on_screen(|window| window.reply());
        }
        None => window.toast(mailrs_domain::translate::gettext(
            "Could not open the conversation to reply to it",
        )),
    }
}

/// Waits up to `wait` for the body a reply to `message` of the
/// conversation `target` would quote. True when the reply may go ahead:
/// the body came or failed, or the wait ran out and the snippet will do,
/// with the conversation still on screen. False once the reader has
/// moved on.
pub(super) async fn body_settles(
    window: &dyn Waiting,
    target: &Target,
    message: Option<&str>,
    wait: Duration,
) -> bool {
    let steps = steps_in(wait);
    for step in 0..=steps {
        if !window.is_showing(target) {
            return false;
        }
        if window.quotable(message) || step == steps {
            return true;
        }
        window.sleep(REVEAL_STEP).await;
    }
    false
}

fn steps_in(wait: Duration) -> u32 {
    (wait.as_millis() / REVEAL_STEP.as_millis().max(1)) as u32
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    /// What the screen shows at one step of the wait: the target, and
    /// whether its body has arrived.
    type Frame = Option<(Target, bool)>;

    /// A window that plays `frames` one per step and then keeps the last.
    struct Window {
        frames: Vec<Frame>,
        at: Cell<usize>,
        replied: RefCell<Vec<Target>>,
        toasts: RefCell<Vec<String>>,
    }

    impl Window {
        fn playing(frames: Vec<Frame>) -> Window {
            Window {
                frames,
                at: Cell::new(0),
                replied: RefCell::new(Vec::new()),
                toasts: RefCell::new(Vec::new()),
            }
        }

        fn frame(&self) -> Frame {
            let at = self.at.get().min(self.frames.len() - 1);
            self.frames[at].clone()
        }
    }

    impl Screen for Window {
        fn is_showing(&self, target: &Target) -> bool {
            self.frame().is_some_and(|(shown, _)| shown == *target)
        }
    }

    impl Waiting for Window {
        fn target(&self) -> Option<Target> {
            self.frame().map(|(target, _)| target)
        }

        fn quotable(&self, _: Option<&str>) -> bool {
            self.frame().is_some_and(|(_, quotable)| quotable)
        }

        fn sleep(&self, _: Duration) -> Answer<'_, ()> {
            self.at.set(self.at.get() + 1);
            Box::pin(async {})
        }

        fn reply(&self) {
            let target = self.target().expect("a reply needs a conversation");
            self.replied.borrow_mut().push(target);
        }

        fn toast(&self, text: String) {
            self.toasts.borrow_mut().push(text);
        }
    }

    fn thread(id: &str) -> Target {
        Target::thread(1, id)
    }

    const WAIT: Duration = Duration::from_millis(500);

    #[tokio::test]
    async fn the_reply_waits_for_the_body_of_the_thread_it_was_asked_for() {
        let window = Window::playing(vec![
            None,
            Some((thread("t1"), false)),
            Some((thread("t1"), false)),
            Some((thread("t1"), true)),
        ]);
        reply_when_open(&window, 1, "t1", WAIT).await;
        assert_eq!(*window.replied.borrow(), vec![thread("t1")]);
        assert_eq!(window.at.get(), 3, "it stopped waiting once the body came");
    }

    #[tokio::test]
    async fn a_reader_who_opens_another_conversation_gets_no_reply_to_it() {
        let window = Window::playing(vec![
            Some((thread("t1"), false)),
            Some((thread("t2"), true)),
        ]);
        reply_when_open(&window, 1, "t1", WAIT).await;
        assert!(window.replied.borrow().is_empty());
        assert!(window.toasts.borrow().is_empty(), "the reader chose that");
    }

    #[tokio::test]
    async fn a_conversation_left_open_from_before_is_not_the_one_answered() {
        // The thread never opens: the row was not in the list.
        let window = Window::playing(vec![Some((thread("t9"), true))]);
        reply_when_open(&window, 1, "t1", WAIT).await;
        assert!(window.replied.borrow().is_empty());
        assert_eq!(window.toasts.borrow().len(), 1);
    }

    #[tokio::test]
    async fn a_slow_body_gives_a_reply_that_quotes_the_snippet() {
        let window = Window::playing(vec![Some((thread("t1"), false))]);
        reply_when_open(&window, 1, "t1", WAIT).await;
        assert_eq!(*window.replied.borrow(), vec![thread("t1")]);
    }

    #[tokio::test]
    async fn a_reply_from_the_button_waits_for_the_body() {
        let window = Window::playing(vec![
            Some((thread("t1"), false)),
            Some((thread("t1"), false)),
            Some((thread("t1"), true)),
        ]);
        assert!(body_settles(&window, &thread("t1"), None, WAIT).await);
        assert_eq!(window.at.get(), 2, "it stopped waiting once the body came");
    }

    #[tokio::test]
    async fn a_reply_from_the_button_is_dropped_when_the_reader_moves_on() {
        let window = Window::playing(vec![
            Some((thread("t1"), false)),
            Some((thread("t2"), true)),
        ]);
        assert!(!body_settles(&window, &thread("t1"), None, WAIT).await);
    }

    #[tokio::test]
    async fn a_body_that_never_comes_lets_the_reply_go_with_the_snippet() {
        let window = Window::playing(vec![Some((thread("t1"), false))]);
        assert!(body_settles(&window, &thread("t1"), None, WAIT).await);
        assert_eq!(window.at.get(), 5, "the whole wait");
    }

    #[tokio::test]
    async fn the_thread_may_open_as_one_of_its_messages() {
        let one = Target {
            message_id: Some("m3".into()),
            ..thread("t1")
        };
        let window = Window::playing(vec![None, Some((one.clone(), true))]);
        reply_when_open(&window, 1, "t1", WAIT).await;
        assert_eq!(*window.replied.borrow(), vec![one]);
    }
}

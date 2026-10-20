//! Which new mail a notification names, and which still deserves one
//! once the local rules ran and the window has been looked at.

use mailrs_domain::{MessageMeta, Target};

/// Whether new mail still deserves its notification once the rules ran:
/// still in the Inbox and still unread.
pub(crate) fn still_news(in_inbox: bool, unread: bool) -> bool {
    in_inbox && unread
}

/// Whether `meta` is in the conversation on screen, `viewed`: a
/// notification about it would only repeat what the window shows. Only
/// the account and thread count, since the thread is on screen whether
/// the view shows all of it or one message.
pub(crate) fn in_view(meta: &MessageMeta, viewed: Option<&Target>) -> bool {
    viewed.is_some_and(|t| t.account_id == meta.account_id && t.thread_id == meta.thread_id)
}

/// The new messages a notification names, oldest first, read in one
/// query. Each used to load its whole thread to pick itself out of it.
pub(crate) fn announced(
    c: &rusqlite::Connection,
    account_id: mailrs_domain::AccountId,
    message_ids: &[String],
) -> mailrs_store::Result<Vec<mailrs_domain::MessageMeta>> {
    mailrs_store::messages::by_ids(c, account_id, message_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_announced_mail_is_the_new_messages_and_not_their_threads() {
        use crate::assistant::run::fake::meta;
        use mailrs_store::messages::{Change, apply};

        let dir = tempfile::tempdir().unwrap();
        let db = mailrs_store::Db::open(&dir.path().join("mail.db")).unwrap();
        let found = db
            .write(|c| {
                let account_id = mailrs_store::accounts::insert_account(c, "me@example.com", 0)?;
                let changes: Vec<Change> = [("a", 1), ("b", 2), ("c", 3)]
                    .into_iter()
                    .map(|(id, at)| Change::Upsert {
                        meta: Box::new(meta(id, "t", "ann@example.com", "Kites", at)),
                        generation: 1,
                    })
                    .collect();
                apply(c, account_id, &changes)?;
                announced(
                    c,
                    account_id,
                    &["c".to_string(), "a".to_string(), "gone".to_string()],
                )
            })
            .await
            .unwrap();
        let ids: Vec<&str> = found.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a", "c"], "oldest first, and nothing the store lost");
    }

    /// The announcement reads its snapshot, then the mail is read on
    /// another device and the change event arrives before the
    /// notification registers, so it finds nothing to settle. The
    /// notification then goes out from the stale snapshot. The second
    /// look `announce_found` takes after registering is what withdraws
    /// it. Each step here is the production piece the app runs.
    #[tokio::test]
    async fn a_read_landing_while_the_announcement_is_pending_settles_at_the_second_look() {
        use crate::notify::{self, Issued};
        use mailrs_domain::mailbox::keyword;
        use mailrs_domain::{MailboxKind, Membership, RemoteMailbox, Role};
        use mailrs_store::messages::{Change, apply, unread_in_inbox};
        use mailrs_sync::fake::meta;

        let dir = tempfile::tempdir().unwrap();
        let db = mailrs_store::Db::open(&dir.path().join("mail.db")).unwrap();
        let account_id = db
            .write(|c| {
                let account_id = mailrs_store::accounts::insert_account(c, "me@example.com", 0)?;
                mailrs_store::mailboxes::upsert(
                    c,
                    account_id,
                    &RemoteMailbox {
                        id: "INBOX".into(),
                        name: "INBOX".into(),
                        kind: MailboxKind::System,
                        role: Some(Role::Inbox),
                        color: None,
                        hidden: false,
                    },
                )?;
                let mut m = meta("m", "t", 1, &["INBOX", "UNREAD"]);
                m.account_id = account_id;
                apply(
                    c,
                    account_id,
                    &[Change::Upsert {
                        meta: Box::new(m),
                        generation: 1,
                    }],
                )?;
                Ok(account_id)
            })
            .await
            .unwrap();
        let asked = vec!["m".to_string()];
        let snapshot = db
            .read(move |c| announced(c, account_id, &asked))
            .await
            .unwrap();
        assert!(
            snapshot[0].is_unread(),
            "the snapshot the announcement works from"
        );

        db.write(move |c| {
            apply(
                c,
                account_id,
                &[Change::of(
                    "m",
                    Membership::Keyword(keyword::SEEN.into()),
                    true,
                )],
            )
        })
        .await
        .unwrap();
        let mut issued = Issued::default();
        assert!(
            issued.in_threads(account_id, &["t".to_string()]).is_empty(),
            "the change event arrives before the notification registers"
        );

        for notice in notify::notices(snapshot, true, vec![]) {
            issued.issue("token".into(), notice.target.unwrap(), notice.buttons);
        }
        let asked = issued.in_threads(account_id, &["t".to_string()]);
        let (token, ids) = asked
            .into_iter()
            .next()
            .expect("the notification waits on the message");
        let still = db
            .read(move |c| unread_in_inbox(c, account_id, &ids))
            .await
            .unwrap();
        let asked = issued.in_threads(account_id, &["t".to_string()]);
        assert!(
            issued.settle(&token, &notify::settled_among(&asked[0].1, &still)),
            "the second look withdraws it"
        );
    }

    #[test]
    fn mail_in_the_conversation_on_screen_is_not_announced() {
        use crate::assistant::run::fake::meta;

        let m = meta("a", "t1", "ann@example.com", "Kites", 1);
        assert!(in_view(&m, Some(&Target::thread(1, "t1"))));
        assert!(!in_view(&m, None), "no conversation is open");
        assert!(
            !in_view(&m, Some(&Target::thread(1, "t2"))),
            "another thread is open"
        );
        assert!(
            !in_view(&m, Some(&Target::thread(2, "t1"))),
            "the same thread id on another account"
        );
        let one = Target {
            account_id: 1,
            thread_id: "t1".into(),
            message_id: Some("b".into()),
        };
        assert!(
            in_view(&m, Some(&one)),
            "the thread is on screen when one message of it is"
        );
    }

    #[test]
    fn mail_a_rule_moved_or_read_is_not_announced() {
        assert!(still_news(true, true));
        assert!(!still_news(false, true), "a rule moved it out of the Inbox");
        assert!(!still_news(true, false), "a rule marked it read");
    }
}

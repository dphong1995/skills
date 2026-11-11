use crate::support::*;
use claude_sessions_core::pull_requests::{
    self, Chime, PRComment, PREvent, PRRef, PRStatus, events, last_other_from_comments,
};
use claude_sessions_core::time::{from_unix_secs, unix_secs};
use serde_json::{Value, json};
use std::collections::HashSet;

const WS: &str = "t6engineering";

fn r(repo: &str, id: i64) -> PRRef {
    PRRef {
        workspace: WS.into(),
        repo: repo.into(),
        id,
    }
}

fn link(repo: &str, id: i64) -> String {
    format!("https://bitbucket.org/{WS}/{repo}/pull-requests/{id}")
}

fn status(o: &Value) -> PRStatus {
    pull_requests::status_from(o).unwrap()
}

fn data(o: &Value) -> Vec<u8> {
    serde_json::to_vec(o).unwrap()
}

fn spec(id: i64) -> PrSpec {
    PrSpec {
        id,
        ..Default::default()
    }
}

#[test]
fn refs_from_text() {
    let text = format!(
        "see {} and {}, again {}",
        link("talo6-dct-backend", 7571),
        link("talo6-dct-frontend", 6874),
        link("talo6-dct-backend", 7571)
    );
    assert_eq!(
        pull_requests::refs(&text),
        [r("talo6-dct-backend", 7571), r("talo6-dct-frontend", 6874)]
    );
    assert!(pull_requests::refs("https://github.com/a/b/pull/3").is_empty());
}

#[test]
fn remote_parsing() {
    let repo = pull_requests::repo_from_remote;
    let ws = |s: &str| (WS.to_string(), s.to_string());
    assert_eq!(
        repo("git@bitbucket.org:t6engineering/talo6-dct-backend.git\n"),
        Some(ws("talo6-dct-backend"))
    );
    assert_eq!(
        repo("https://phong@bitbucket.org/t6engineering/talos-v2.git"),
        Some(ws("talos-v2"))
    );
    assert_eq!(
        repo("https://bitbucket.org/t6engineering/talos-v2"),
        Some(ws("talos-v2"))
    );
    assert_eq!(repo("git@github.com:a/b.git"), None);
}

#[test]
fn status_from_list_value() {
    let s = status(&pr_json(PrSpec {
        id: 7571,
        comments: 23,
        reviewers: 5,
        ..Default::default()
    }));
    assert!(s.reviewers == 5 && s.approved == 1 && s.changes_requested == 1 && s.comments == 23);
    assert_eq!(s.branch.as_deref(), Some("bugfix/T6IR-14744"));
    assert_eq!(
        s.source_repo.as_deref(),
        Some("t6engineering/talo6-dct-backend")
    );
    assert!(s.is_open() && s.ref_ == r("talo6-dct-backend", 7571));
    assert_eq!(s.commit.as_deref(), Some("abc123"));
    assert_eq!(s.last_other, None);
    assert!(pull_requests::status_from(&json!({"state": "OPEN"})).is_none());
}

#[test]
fn page_with_next() {
    let p = pull_requests::page(&data(
        &json!({"values": [pr_json(spec(1)), pr_json(spec(2))], "next": "https://x/next"}),
    ))
    .unwrap();
    assert!(p.values.len() == 2 && p.next.as_deref() == Some("https://x/next"));
    assert_eq!(
        pull_requests::page(&data(&json!({"values": []})))
            .unwrap()
            .next,
        None
    );
    assert!(pull_requests::page(b"nope").is_none());
}

#[test]
fn last_other_skips_me_and_deleted() {
    let c = |uuid: &str, name: &str, at: &str, deleted: bool| json!({"created_on": at, "user": {"uuid": uuid, "display_name": name}, "deleted": deleted});
    let d = data(&json!({"values": [
        c("{me}", "Me", "2026-11-04T06:00:00.000000+00:00", false),
        c("{x}", "Gone", "2026-11-04T05:30:00.000000+00:00", true),
        c("{t}", "Tin Tran", "2026-11-04T05:02:17.239112+00:00", false)]}));
    let got = last_other_from_comments(&d, "{me}").unwrap();
    assert_eq!(got.name, "Tin Tran");
    assert!((unix_secs(got.at) - 1_793_768_537.239).abs() < 0.01);
    let plain = last_other_from_comments(
        &data(&json!({"values": [c("{t}", "T", "2026-11-04T05:02:17Z", false)]})),
        "{me}",
    )
    .unwrap();
    assert_eq!(plain.at, from_unix_secs(1_793_768_537.0));
}

#[test]
fn events_each_kind() {
    let base = status(&pr_json(PrSpec {
        id: 1,
        reviewers: 3,
        ..Default::default()
    }));
    assert_eq!(events(&base, &base), []);
    let mut with_comment = base.clone();
    with_comment.last_other = Some(PRComment {
        name: "A".into(),
        at: from_unix_secs(100.0),
    });
    assert_eq!(events(&base, &with_comment), [PREvent::Comment]);
    let mut newer = with_comment.clone();
    newer.last_other = Some(PRComment {
        name: "B".into(),
        at: from_unix_secs(200.0),
    });
    assert_eq!(events(&with_comment, &newer), [PREvent::Comment]);
    assert_eq!(events(&newer, &with_comment), []);

    let with = |state: &'static str, reviewers: usize| {
        status(&pr_json(PrSpec {
            id: 1,
            state,
            reviewers,
            ..Default::default()
        }))
    };
    let fewer = with("OPEN", 1); // 0 approved, 1 changes_requested
    let more = with("OPEN", 3); // 1 approved, 1 changes_requested
    assert_eq!(events(&fewer, &more), [PREvent::Approved]);
    let none = with("OPEN", 0);
    assert_eq!(events(&none, &fewer), [PREvent::ChangesRequested]);

    assert_eq!(events(&base, &with("MERGED", 3)), [PREvent::Merged]);
    assert_eq!(events(&base, &with("DECLINED", 3)), [PREvent::Declined]);
    assert_eq!(events(&base, &with("SUPERSEDED", 3)), [PREvent::Declined]);
    let merged = with("MERGED", 3);
    assert_eq!(events(&merged, &with("DECLINED", 3)), []);
}

#[test]
fn chime_priority() {
    use PREvent::*;
    let chime = pull_requests::chime;
    assert_eq!(chime(&[Approved, Comment]), Some(Chime::NeedsYou));
    assert_eq!(chime(&[Merged]), Some(Chime::Done));
    assert_eq!(chime(&[Approved]), Some(Chime::Done));
    assert_eq!(chime(&[Declined]), Some(Chime::NeedsYou));
    assert_eq!(chime(&[]), None);
}

#[test]
fn last_other_skips_code_rabbit() {
    let bot = rabbit_comment(
        "**Actionable comments posted: 1**",
        "2026-11-03T05:00:00.000000+00:00",
    );
    let human = comment("{p}", "Phong Luu", "2026-11-03T04:00:00.000000+00:00");
    let c = last_other_from_comments(&data(&json!({"values": [bot, human]})), "{me}");
    assert_eq!(c.unwrap().name, "Phong Luu");
}

#[test]
fn chime_rabbit_events() {
    use PREvent::*;
    let chime = pull_requests::chime;
    assert_eq!(chime(&[RabbitDone]), Some(Chime::Done));
    assert_eq!(chime(&[RabbitNeedsYou]), Some(Chime::NeedsYou));
    assert_eq!(chime(&[RabbitDone, RabbitNeedsYou]), Some(Chime::NeedsYou));
}

#[test]
fn prompt_template() {
    let url = url::Url::parse(&link("talo6-dct-backend", 7571)).unwrap();
    assert_eq!(
        pull_requests::prompt("Fix {url} (#{id})\nnow", &url, 7571),
        format!("Fix {} (#7571) now", url.as_str())
    );
    assert_eq!(
        pull_requests::prompt("  ", &url, 7571),
        pull_requests::DEFAULT_PROMPT.replace("{url}", url.as_str())
    );
}

#[test]
fn status_reads_title() {
    let mut o = pr_json(spec(1));
    o["title"] = json!("Fix site scope filter");
    assert_eq!(
        pull_requests::status_from(&o).unwrap().title.as_deref(),
        Some("Fix site scope filter")
    );
    o.as_object_mut().unwrap().remove("title");
    assert_eq!(pull_requests::status_from(&o).unwrap().title, None);
}

#[test]
fn approvers_from_participants() {
    let set = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<HashSet<_>>();
    let pr = |rabbit_approved| {
        pull_requests::status_from(&pr_json(PrSpec {
            id: 1,
            reviewers: 2,
            rabbit_approved: Some(rabbit_approved),
            ..Default::default()
        }))
        .unwrap()
    };
    assert_eq!(pr(true).approvers, set(&["{r1}", "{tin}"]));
    assert_eq!(pr(false).approvers, set(&["{r1}"]));
}

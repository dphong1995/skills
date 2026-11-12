//! The Settings window's model: what each action changes and asks for.

use claude_sessions::hotkey::Which;
use claude_sessions::settings::BG_OPACITIES;
use claude_sessions::settings_model::*;
use claude_sessions_core::credentials::Service::{Bitbucket, Jira};
use claude_sessions_core::pull_requests::DEFAULT_PROMPT;
use claude_sessions_core::scene::{DrawCmd, ThemeColor};
use claude_sessions_core::store::{ConnectionLine, Paths, Store, StoreOptions};

fn store(dir: &std::path::Path) -> Store {
    Store::new(
        Paths {
            sessions: dir.join("sessions"),
            projects: dir.join("projects"),
            tasks: dir.join("tasks"),
        },
        StoreOptions::default(),
    )
}

fn setup() -> (tempfile::TempDir, Store, SettingsModel) {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    s.accounts.insert(Bitbucket, "bb@x".into());
    let m = SettingsModel::new(&s, DEFAULT_PROMPT, true);
    (dir, s, m)
}

fn learn(store: &mut Store, host: &str) {
    store.jira_projects.insert("A".into(), host.into());
}

#[test]
fn opacity_and_mute_apply_at_once_and_persist() {
    let (_d, mut s, mut m) = setup();
    let e = apply(&mut m, &mut s, SettingsAction::SetOpacity(BG_OPACITIES[3]));
    assert_eq!(e, vec![SettingsEffect::Persist]);
    assert_eq!(s.bg_opacity, BG_OPACITIES[3]);
    let e = apply(&mut m, &mut s, SettingsAction::ToggleMute);
    assert_eq!(e, vec![SettingsEffect::Persist]);
    assert!(s.muted);
    apply(&mut m, &mut s, SettingsAction::ToggleMute);
    assert!(!s.muted);
}

#[test]
fn pick_hotkey_asks_for_the_popup() {
    let (_d, mut s, mut m) = setup();
    let e = apply(&mut m, &mut s, SettingsAction::PickHotkey(Which::Next));
    assert_eq!(e, vec![SettingsEffect::PopupHotkey(Which::Next)]);
}

#[test]
fn prompt_commit_falls_back_to_default_when_empty() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::PromptChanged("  mine  ".into()),
    );
    let e = apply(&mut m, &mut s, SettingsAction::PromptCommit);
    assert_eq!(e, vec![SettingsEffect::SetPrompt("mine".into())]);
    apply(&mut m, &mut s, SettingsAction::PromptChanged("  ".into()));
    let e = apply(&mut m, &mut s, SettingsAction::PromptCommit);
    assert_eq!(e, vec![SettingsEffect::SetPrompt(DEFAULT_PROMPT.into())]);
    assert_eq!(m.prompt, DEFAULT_PROMPT);
    assert_eq!(prompt_value("", "d"), "d");
}

#[test]
fn prompt_reset_restores_the_default() {
    let (_d, mut s, mut m) = setup();
    apply(&mut m, &mut s, SettingsAction::PromptChanged("mine".into()));
    let e = apply(&mut m, &mut s, SettingsAction::PromptReset);
    assert_eq!(e, vec![SettingsEffect::SetPrompt(DEFAULT_PROMPT.into())]);
    assert_eq!(m.prompt, DEFAULT_PROMPT);
}

#[test]
fn normalize_site_cases() {
    assert_eq!(
        normalize_site(" https://cong-ty.atlassian.net/ ").as_deref(),
        Some("cong-ty.atlassian.net")
    );
    assert_eq!(
        normalize_site("http://a.atlassian.net").as_deref(),
        Some("a.atlassian.net")
    );
    assert_eq!(normalize_site(""), None);
    assert_eq!(normalize_site("  "), None);
}

#[test]
fn normalize_site_keeps_only_the_host() {
    assert_eq!(
        normalize_site("https://cong-ty.atlassian.net/jira/your-work?x=1").as_deref(),
        Some("cong-ty.atlassian.net")
    );
    assert_eq!(
        normalize_site("Cong-Ty.Atlassian.net/browse/A-1").as_deref(),
        Some("cong-ty.atlassian.net")
    );
    assert_eq!(
        normalize_site("evil.com/.atlassian.net").as_deref(),
        Some("evil.com")
    );
}

#[test]
fn check_accepts_a_pasted_url_with_a_path() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("https://cong-ty.atlassian.net/jira/your-work".into()),
    );
    let e = apply(&mut m, &mut s, SettingsAction::Check(Jira));
    assert_eq!(
        e,
        vec![SettingsEffect::RunCheck {
            service: Jira,
            email: String::new(),
            site: Some("cong-ty.atlassian.net".into())
        }]
    );
}

#[test]
fn check_refuses_a_site_that_hides_another_host_behind_a_path() {
    let (_d, mut s, mut m) = setup();
    for typed in [
        "evil.com/.atlassian.net",
        "a.b.com#.atlassian.net",
        "a b.atlassian.net",
    ] {
        apply(&mut m, &mut s, SettingsAction::SiteChanged(typed.into()));
        let e = apply(&mut m, &mut s, SettingsAction::Check(Jira));
        assert!(e.is_empty(), "{typed}");
        assert_eq!(
            m.jira.check,
            CheckState::Failed("✗ Site phải có dạng <tên>.atlassian.net".into()),
            "{typed}"
        );
    }
}

#[test]
fn site_commit_saves_only_an_atlassian_site() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("evil.example.com".into()),
    );
    let e = apply(&mut m, &mut s, SettingsAction::SiteCommit);
    assert!(e.is_empty());
    assert_eq!(s.jira_site, None);
    assert_eq!(m.site, "evil.example.com");
    assert_eq!(
        m.jira.check,
        CheckState::Failed("✗ Site phải có dạng <tên>.atlassian.net".into())
    );
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("evil.com/.atlassian.net".into()),
    );
    assert!(apply(&mut m, &mut s, SettingsAction::SiteCommit).is_empty());
    assert_eq!(s.jira_site, None);
}

#[test]
fn site_commit_keeps_a_running_check() {
    let (_d, mut s, mut m) = setup();
    learn(&mut s, "a.atlassian.net");
    apply(&mut m, &mut s, SettingsAction::Check(Jira));
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("evil.com".into()),
    );
    apply(&mut m, &mut s, SettingsAction::SiteCommit);
    assert_eq!(m.jira.check, CheckState::Checking);
}

#[test]
fn leaving_the_site_field_after_return_writes_once() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("b.atlassian.net".into()),
    );
    let ret = submit_action(SITE_KEY).unwrap();
    let blur = submit_action(SITE_KEY).unwrap();
    assert_eq!(apply(&mut m, &mut s, ret), vec![SettingsEffect::Persist]);
    assert!(apply(&mut m, &mut s, blur).is_empty());
}

#[test]
fn close_commits_the_typed_site_and_prompt() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("b.atlassian.net".into()),
    );
    apply(&mut m, &mut s, SettingsAction::PromptChanged("mine".into()));
    let e = apply(&mut m, &mut s, SettingsAction::Close);
    assert_eq!(s.jira_site.as_deref(), Some("b.atlassian.net"));
    assert_eq!(
        e,
        vec![
            SettingsEffect::SetPrompt("mine".into()),
            SettingsEffect::Persist,
            SettingsEffect::CloseWindow
        ]
    );
}

#[test]
fn close_without_editing_the_prompt_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = store(dir.path());
    let mut m = SettingsModel::new(&s, "  stored with spaces \n", true);
    let e = apply(&mut m, &mut s, SettingsAction::Close);
    assert_eq!(e, vec![SettingsEffect::CloseWindow]);
    // A real edit is still saved, once.
    apply(&mut m, &mut s, SettingsAction::PromptChanged("mine".into()));
    let e = apply(&mut m, &mut s, SettingsAction::PromptCommit);
    assert_eq!(e, vec![SettingsEffect::SetPrompt("mine".into())]);
    assert!(apply(&mut m, &mut s, SettingsAction::PromptCommit).is_empty());
    let e = apply(&mut m, &mut s, SettingsAction::PromptReset);
    assert_eq!(e, vec![SettingsEffect::SetPrompt(DEFAULT_PROMPT.into())]);
}

#[test]
fn a_job_in_flight_before_the_window_opened_shows_and_blocks() {
    let (_d, mut s, mut m) = setup();
    m.resume(Bitbucket, Job::Save);
    assert_eq!(m.bitbucket.check, CheckState::Saving);
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Bitbucket, true));
    assert!(apply(&mut m, &mut s, SettingsAction::Save(Bitbucket)).is_empty());
    assert!(apply(&mut m, &mut s, SettingsAction::Check(Bitbucket)).is_empty());
    assert_eq!(m.bitbucket.check, CheckState::Saving);
    m.resume(Bitbucket, Job::Check);
    assert_eq!(m.bitbucket.check, CheckState::Checking);
    m.done(Bitbucket, Job::Check, Ok("bb@x".into()));
    assert!(matches!(m.bitbucket.check, CheckState::Ok(_)));
}

#[test]
fn note_field_keeps_the_token_text_apart_from_the_model() {
    let mut tokens = std::collections::HashMap::new();
    let key = token_key(Jira, 0);
    let a = note_field(&mut tokens, &key, "s3cr3t");
    assert_eq!(a, Some(SettingsAction::TokenEdited(Jira, true)));
    assert_eq!(tokens.get(&Jira).map(String::as_str), Some("s3cr3t"));
    let a = note_field(&mut tokens, &key, "s3cr3t2");
    assert_eq!(a, Some(SettingsAction::TokenEdited(Jira, true)));
    assert_eq!(tokens.get(&Jira).map(String::as_str), Some("s3cr3t2"));
    note_field(&mut tokens, &token_key(Bitbucket, 0), "other");
    let a = note_field(&mut tokens, &key, "");
    assert_eq!(a, Some(SettingsAction::TokenEdited(Jira, false)));
    assert!(!tokens.contains_key(&Jira));
    assert!(tokens.contains_key(&Bitbucket));
}

#[test]
fn note_field_leaves_the_tokens_alone_for_other_fields() {
    let mut tokens = std::collections::HashMap::new();
    tokens.insert(Jira, "keep".to_string());
    for (key, text) in [
        (PROMPT_KEY.to_string(), "p"),
        (SITE_KEY.to_string(), ""),
        (email_key(Jira), "e"),
        ("other".to_string(), "x"),
    ] {
        note_field(&mut tokens, &key, text);
    }
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.get(&Jira).map(String::as_str), Some("keep"));
    assert_eq!(
        note_field(&mut tokens, &email_key(Jira), "e"),
        Some(SettingsAction::EmailChanged(Jira, "e".into()))
    );
    assert_eq!(note_field(&mut tokens, "other", "x"), None);
}

#[test]
fn typing_actions_apply_without_effects() {
    // The app applies these at once inside the field event and renders later; that is only safe while they ask for nothing.
    let (_d, mut s, mut m) = setup();
    for (key, text) in [
        (PROMPT_KEY.to_string(), "p"),
        (SITE_KEY.to_string(), "s"),
        (email_key(Jira), "e"),
        (token_key(Jira, 0), "t"),
    ] {
        let a = changed_action(&key, text).unwrap();
        assert!(apply(&mut m, &mut s, a).is_empty(), "{key}");
    }
    assert_eq!(m.prompt, "p");
    assert_eq!(m.site, "s");
}

#[test]
fn site_commit_saves_the_typed_site() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("https://b.atlassian.net/".into()),
    );
    let e = apply(&mut m, &mut s, SettingsAction::SiteCommit);
    assert_eq!(e, vec![SettingsEffect::Persist]);
    assert_eq!(s.jira_site.as_deref(), Some("b.atlassian.net"));
    assert!(apply(&mut m, &mut s, SettingsAction::SiteCommit).is_empty());
    apply(&mut m, &mut s, SettingsAction::SiteChanged("".into()));
    let e = apply(&mut m, &mut s, SettingsAction::SiteCommit);
    assert_eq!(e, vec![SettingsEffect::Persist]);
    assert_eq!(s.jira_site, None);
}

#[test]
fn close_without_typing_a_site_saves_no_site() {
    let (_d, mut s, mut m) = setup();
    learn(&mut s, "a.atlassian.net");
    let e = apply(&mut m, &mut s, SettingsAction::Close);
    assert_eq!(s.jira_site, None);
    assert!(!e.contains(&SettingsEffect::Persist));
    assert_eq!(e.last(), Some(&SettingsEffect::CloseWindow));
}

#[test]
fn check_with_an_empty_token_runs_on_the_saved_one() {
    let (_d, mut s, mut m) = setup();
    let e = apply(&mut m, &mut s, SettingsAction::Check(Bitbucket));
    assert_eq!(m.bitbucket.check, CheckState::Checking);
    assert_eq!(
        e,
        vec![SettingsEffect::RunCheck {
            service: Bitbucket,
            email: "bb@x".into(),
            site: None
        }]
    );
}

#[test]
fn check_runs_jira_on_the_guess_or_the_typed_site() {
    let (_d, mut s, mut m) = setup();
    learn(&mut s, "a.atlassian.net");
    apply(
        &mut m,
        &mut s,
        SettingsAction::EmailChanged(Jira, " me@x ".into()),
    );
    let e = apply(&mut m, &mut s, SettingsAction::Check(Jira));
    assert_eq!(
        e,
        vec![SettingsEffect::RunCheck {
            service: Jira,
            email: "me@x".into(),
            site: Some("a.atlassian.net".into())
        }]
    );
}

#[test]
fn check_without_a_jira_site_fails() {
    let (_d, mut s, mut m) = setup();
    let e = apply(&mut m, &mut s, SettingsAction::Check(Jira));
    assert!(e.is_empty());
    assert_eq!(
        m.jira.check,
        CheckState::Failed("✗ Chưa có site Jira".into())
    );
}

#[test]
fn check_refuses_a_non_atlassian_site() {
    let (_d, mut s, mut m) = setup();
    apply(
        &mut m,
        &mut s,
        SettingsAction::SiteChanged("evil.example.com".into()),
    );
    let e = apply(&mut m, &mut s, SettingsAction::Check(Jira));
    assert!(e.is_empty());
    assert_eq!(
        m.jira.check,
        CheckState::Failed("✗ Site phải có dạng <tên>.atlassian.net".into())
    );
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Jira, true));
    assert!(apply(&mut m, &mut s, SettingsAction::Save(Jira)).is_empty());
}

#[test]
fn check_and_save_refuse_in_a_test_run() {
    let (_d, mut s, _) = setup();
    let mut m = SettingsModel::new(&s, DEFAULT_PROMPT, false);
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Bitbucket, true));
    for a in [
        SettingsAction::Check(Bitbucket),
        SettingsAction::Save(Bitbucket),
    ] {
        assert!(apply(&mut m, &mut s, a).is_empty());
        assert_eq!(
            m.bitbucket.check,
            CheckState::Failed("✗ Bản chạy thử không dùng Keychain".into())
        );
    }
}

#[test]
fn save_without_a_typed_token_fails_without_work() {
    let (_d, mut s, mut m) = setup();
    let e = apply(&mut m, &mut s, SettingsAction::Save(Bitbucket));
    assert!(e.is_empty());
    assert_eq!(
        m.bitbucket.check,
        CheckState::Failed("✗ Nhập token mới để lưu".into())
    );
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Bitbucket, true));
    let e = apply(&mut m, &mut s, SettingsAction::Save(Bitbucket));
    assert_eq!(m.bitbucket.check, CheckState::Saving);
    assert!(matches!(
        &e[..],
        [SettingsEffect::RunSave {
            service: Bitbucket,
            ..
        }]
    ));
}

#[test]
fn a_busy_service_ignores_check_and_save() {
    let (_d, mut s, mut m) = setup();
    apply(&mut m, &mut s, SettingsAction::Check(Bitbucket));
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Bitbucket, true));
    assert_eq!(m.bitbucket.check, CheckState::Checking);
    assert!(apply(&mut m, &mut s, SettingsAction::Check(Bitbucket)).is_empty());
    assert!(apply(&mut m, &mut s, SettingsAction::Save(Bitbucket)).is_empty());
    assert_eq!(m.bitbucket.check, CheckState::Checking);
}

#[test]
fn save_success_clears_the_token_field() {
    let (_d, mut s, mut m) = setup();
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Bitbucket, true));
    apply(&mut m, &mut s, SettingsAction::Save(Bitbucket));
    m.done(Bitbucket, Job::Save, Ok("new@x".into()));
    let f = &m.bitbucket;
    assert_eq!(f.token_gen, 1);
    assert!(!f.token_typed);
    assert_eq!(f.check, CheckState::Ok("✓ Đã lưu vào Keychain".into()));
    assert_eq!(f.verified.as_deref(), Some("new@x"));
    assert_eq!(f.email, "new@x");
}

#[test]
fn check_success_names_the_account() {
    let (_d, mut s, mut m) = setup();
    apply(&mut m, &mut s, SettingsAction::Check(Bitbucket));
    m.done(Bitbucket, Job::Check, Ok("bb@x".into()));
    assert_eq!(
        m.bitbucket.check,
        CheckState::Ok("✓ Token dùng được · bb@x".into())
    );
    assert_eq!(m.bitbucket.token_gen, 0);
}

#[test]
fn a_failure_shows_its_text() {
    let (_d, mut s, mut m) = setup();
    apply(&mut m, &mut s, SettingsAction::Check(Bitbucket));
    m.done(Bitbucket, Job::Check, Err("✗ Lỗi HTTP 500".into()));
    assert_eq!(
        m.bitbucket.check,
        CheckState::Failed("✗ Lỗi HTTP 500".into())
    );
}

#[test]
fn editing_a_field_clears_the_result() {
    let (_d, mut s, mut m) = setup();
    m.done(Jira, Job::Check, Err("x".into()));
    apply(&mut m, &mut s, SettingsAction::SiteChanged("a".into()));
    assert_eq!(m.jira.check, CheckState::Idle);
    m.done(Bitbucket, Job::Check, Err("x".into()));
    apply(
        &mut m,
        &mut s,
        SettingsAction::EmailChanged(Bitbucket, "q".into()),
    );
    assert_eq!(m.bitbucket.check, CheckState::Idle);
    m.done(Bitbucket, Job::Check, Err("x".into()));
    apply(&mut m, &mut s, SettingsAction::TokenEdited(Bitbucket, true));
    assert_eq!(m.bitbucket.check, CheckState::Idle);
    // A busy service keeps its state.
    apply(&mut m, &mut s, SettingsAction::Check(Bitbucket));
    apply(
        &mut m,
        &mut s,
        SettingsAction::EmailChanged(Bitbucket, "z".into()),
    );
    assert_eq!(m.bitbucket.check, CheckState::Checking);
}

#[test]
fn status_line_shows_the_check_result_while_the_monitor_is_checking() {
    let (_d, mut s, mut m) = setup();
    s.set_connection(Jira, ConnectionLine::Checking("a".into()));
    assert_eq!(
        status_line(&m, &s, Jira),
        ("a (đang kiểm tra)".to_string(), ThemeColor::TextSecondary)
    );
    m.jira.verified = Some("a@x".into());
    assert_eq!(
        status_line(&m, &s, Jira),
        ("✓ a@x".to_string(), ThemeColor::Green)
    );
    s.set_connection(Jira, ConnectionLine::Ok("a".into()));
    assert_eq!(
        status_line(&m, &s, Jira),
        ("✓ a".to_string(), ThemeColor::Green)
    );
    s.set_connection(Jira, ConnectionLine::Rejected);
    assert_eq!(
        status_line(&m, &s, Jira),
        ("✗ token bị từ chối (401)".to_string(), ThemeColor::Red)
    );
}

#[test]
fn field_keys_map_to_actions() {
    assert_eq!(
        changed_action(PROMPT_KEY, "p"),
        Some(SettingsAction::PromptChanged("p".into()))
    );
    assert_eq!(
        changed_action(SITE_KEY, "s"),
        Some(SettingsAction::SiteChanged("s".into()))
    );
    assert_eq!(
        changed_action(&email_key(Bitbucket), "e"),
        Some(SettingsAction::EmailChanged(Bitbucket, "e".into()))
    );
    assert_eq!(
        changed_action(&token_key(Jira, 3), "t"),
        Some(SettingsAction::TokenEdited(Jira, true))
    );
    assert_eq!(
        changed_action(&token_key(Jira, 3), ""),
        Some(SettingsAction::TokenEdited(Jira, false))
    );
    assert_eq!(changed_action("other", "x"), None);
    assert_eq!(
        submit_action(PROMPT_KEY),
        Some(SettingsAction::PromptCommit)
    );
    assert_eq!(submit_action(SITE_KEY), Some(SettingsAction::SiteCommit));
    assert_eq!(submit_action(&email_key(Jira)), None);
    assert_eq!(token_service(&token_key(Bitbucket, 12)), Some(Bitbucket));
    assert_eq!(token_service(&email_key(Bitbucket)), None);
    assert_eq!(token_key(Jira, 0), "settings.token.jira.0");
    assert_eq!(email_key(Bitbucket), "settings.email.bitbucket");
}

#[test]
fn typed_token_stays_out_of_scene_model_and_debug() {
    use claude_sessions::hotkey::Hotkeys;
    use claude_sessions::views::settings::{SettingsView, build};
    use claude_sessions_core::kv::MemoryStore;

    let (_d, mut s, mut m) = setup();
    learn(&mut s, "a.atlassian.net");
    let action = changed_action(&token_key(Jira, 0), "s3cr3t-tok").unwrap();
    assert_eq!(action, SettingsAction::TokenEdited(Jira, true));
    assert!(!format!("{action:?}").contains("s3cr3t"));
    let mut effects = apply(&mut m, &mut s, action);
    effects.extend(apply(&mut m, &mut s, SettingsAction::Save(Jira)));
    assert!(matches!(&effects[..], [SettingsEffect::RunSave { .. }]));
    assert!(!format!("{m:?}{effects:?}").contains("s3cr3t"));
    let hotkeys = Hotkeys::load(&MemoryStore::default());
    let built = build(&SettingsView {
        model: &m,
        store: &s,
        hotkeys: &hotkeys,
        measure: &crate::support::FakeMeasure,
        max_h: 2000.0,
    });
    for n in &built.scene.nodes {
        assert!(!format!("{:?}{:?}", n.fields, n.tips).contains("s3cr3t"));
        for c in &n.cmds {
            if let DrawCmd::Text { text, .. } = c {
                assert!(!text.contains("s3cr3t"));
            }
        }
    }
}

#[test]
fn external_key_changes_only_with_connections_accounts_or_failed_hotkeys() {
    let (_d, mut s, _m) = setup();
    let none = std::collections::BTreeSet::new();
    let base = external_key(&s, &none);
    assert_eq!(base, external_key(&s, &none));
    s.set_connection(Jira, ConnectionLine::Rejected);
    let rejected = external_key(&s, &none);
    assert_ne!(base, rejected);
    s.accounts.insert(Jira, "j@x".into());
    let with_account = external_key(&s, &none);
    assert_ne!(rejected, with_account);
    let failed = std::collections::BTreeSet::from(["open"]);
    assert_ne!(with_account, external_key(&s, &failed));
}

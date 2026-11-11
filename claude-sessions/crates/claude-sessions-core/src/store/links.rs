//! Ticket and PR buttons of a session, and the inline menu built from them.

use super::model::{ItemTone, LinkItem, LinkKind, LinkSection, SessionLink};
use crate::jira::JiraIssue;
use crate::jira_monitor::JiraTarget;
use crate::pull_requests::{self, PRRef, PRStatus};
use crate::transcript::{label_for, project_prefix};
use crate::transcript_scanner::ScanResult;
use std::collections::{HashMap, HashSet};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    User,
    Claude,
    Pr,
}

#[derive(Clone)]
pub(super) struct ButtonEntry {
    pub link: SessionLink,
    pub refs: Vec<PRRef>,
    pub source: Source,
    /// The Jira issue of a ticket button; every ticket button has a known host.
    pub target: Option<JiraTarget>,
}

fn plain(label: String, url: Url) -> SessionLink {
    SessionLink {
        label,
        url,
        help: None,
        icon_url: None,
        tone: None,
    }
}

/// Buttons for one session: tickets the user mentioned, tickets only Claude mentioned, then PR-shaped links.
pub(super) fn buttons(r: &ScanResult, projects: &HashMap<String, String>) -> Vec<ButtonEntry> {
    let host = |key: &str| project_prefix(key).and_then(|p| projects.get(&p).cloned());
    let known: Vec<_> = r
        .tickets
        .values()
        .filter(|t| host(&t.key).is_some())
        .collect();
    let mut user: Vec<_> = known.iter().filter(|t| t.by_user).collect();
    user.sort_by(|a, b| (a.first, &a.key).cmp(&(b.first, &b.key)));
    let mut claude: Vec<_> = known.iter().filter(|t| !t.by_user).collect();
    claude.sort_by(|a, b| (b.last, &a.key).cmp(&(a.last, &b.key)));
    let keys = user
        .iter()
        .map(|t| (t.key.clone(), Source::User))
        .chain(claude.iter().map(|t| (t.key.clone(), Source::Claude)));
    let mut out: Vec<ButtonEntry> = keys
        .filter_map(|(key, source)| {
            let h = host(&key)?;
            let url = Url::parse(&format!("https://{h}/browse/{key}")).ok()?;
            Some(ButtonEntry {
                link: plain(key.clone(), url),
                refs: Vec::new(),
                source,
                target: Some(JiraTarget { key, host: h }),
            })
        })
        .collect();
    out.extend(r.pr_links.iter().map(|u| ButtonEntry {
        link: plain(label_for(u), u.clone()),
        refs: pull_requests::refs(u.as_str()),
        source: Source::Pr,
        target: None,
    }));
    out
}

/// `KEY · title · status` for a ticket button's hover text; `None` until Jira answered with a title.
pub(super) fn help(key: &str, issue: Option<&JiraIssue>) -> Option<String> {
    let issue = issue?;
    let title = issue.title.as_ref()?;
    let mut parts = vec![key, title.as_str()];
    if let Some(s) = &issue.status {
        parts.push(s);
    }
    Some(parts.join(" · "))
}

/// `KEY  title  ·  status`, the title cut at 60 characters; the key alone without a title.
pub(super) fn ticket_text(key: &str, issue: Option<&JiraIssue>) -> String {
    let Some(title) = issue.and_then(|i| i.title.as_ref()) else {
        return key.to_string();
    };
    let cut = if title.chars().count() > 60 {
        format!("{}…", title.chars().take(60).collect::<String>())
    } else {
        title.clone()
    };
    let status = issue
        .and_then(|i| i.status.as_ref())
        .map(|s| format!("  ·  {s}"))
        .unwrap_or_default();
    format!("{key}  {cut}{status}")
}

/// `PR #id  title  ·  state`.
pub fn link_text(pr: &PRStatus) -> String {
    let title = pr
        .title
        .as_ref()
        .map_or("·  ".to_string(), |t| format!("{t}  ·  "));
    format!("PR #{}  {}{}", pr.ref_.id, title, pr.state_word())
}

fn pr_tone(pr: &PRStatus) -> ItemTone {
    if pr.is_open() {
        ItemTone::Open
    } else if pr.state == "MERGED" {
        ItemTone::Merged
    } else {
        ItemTone::Closed
    }
}

/// The menu row of one tracked PR (Swift `Store.menu` for a single PR).
pub fn pr_item(pr: &PRStatus) -> LinkItem {
    LinkItem {
        text: link_text(pr),
        url: pr.url.clone(),
        kind: LinkKind::Pr,
        code: format!("#{}", pr.ref_.id),
        title: pr.title.clone(),
        status: Some(pr.state_word().to_string()),
        tone: Some(pr_tone(pr)),
        icon_url: None,
        help: pr.title.clone(),
    }
}

/// The inline menu: tickets by origin, then every PR. `prs` is `None` without a PR monitor, so every PR link is listed as it is.
pub(super) fn menu(
    entries: &[ButtonEntry],
    issues: &HashMap<String, JiraIssue>,
    prs: Option<&[PRStatus]>,
) -> Vec<LinkSection> {
    let tickets = |source: Source, title: &str| LinkSection {
        title: title.to_string(),
        items: entries
            .iter()
            .filter(|e| e.source == source)
            .filter_map(|e| {
                let t = e.target.as_ref()?;
                let issue = issues.get(&t.key);
                Some(LinkItem {
                    text: ticket_text(&t.key, issue),
                    url: e.link.url.clone(),
                    kind: LinkKind::Ticket,
                    code: t.key.clone(),
                    title: issue.and_then(|i| i.title.clone()),
                    status: issue.and_then(|i| i.status.clone()),
                    tone: issue.and_then(|i| i.tone).map(ItemTone::from),
                    icon_url: issue.and_then(|i| i.icon_url.clone()),
                    help: help(&t.key, issue),
                })
            })
            .collect(),
    };
    let prs = prs.unwrap_or(&[]);
    let tracked: HashSet<&PRRef> = prs.iter().map(|p| &p.ref_).collect();
    let untracked = entries
        .iter()
        .filter(|e| e.source == Source::Pr && !e.refs.iter().any(|r| tracked.contains(r)))
        .map(|e| LinkItem {
            text: e.link.label.clone(),
            url: e.link.url.clone(),
            kind: LinkKind::Link,
            code: e.link.label.clone(),
            title: None,
            status: None,
            tone: None,
            icon_url: None,
            help: None,
        });
    let pr = LinkSection {
        title: "PR".into(),
        items: prs.iter().map(pr_item).chain(untracked).collect(),
    };
    vec![
        tickets(Source::User, "Bạn nhắc"),
        tickets(Source::Claude, "Claude nhắc"),
        pr,
    ]
    .into_iter()
    .filter(|s| !s.items.is_empty())
    .collect()
}

//! One refresh: reads the session files, derives every row field, updates marks, closed list and caches.

use super::*;

impl Store {
    pub fn refresh(&mut self, at: SystemTime) {
        self.now = at;
        let now = at;
        self.prune_sends();
        if let Some(reader) = &mut self.quota_reader {
            let q = reader.read(at);
            if q != self.quota {
                self.quota = q;
            }
        }
        self.chime = None;
        self.transitions.clear();
        self.pr_chime = None;
        let files: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        let mut list: Vec<Session> = Vec::new();
        let mut sizes: HashMap<String, u64> = HashMap::new();
        let mut seen_files: HashSet<PathBuf> = HashSet::new();
        for file in files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == "json"))
        {
            seen_files.insert(file.clone());
            let Some(stamp) = FileStamp::new(file) else {
                continue;
            };
            let (base, spare) = match self.session_json.get(file) {
                Some(c) if c.stamp == stamp => (c.base.clone(), c.spare),
                _ => {
                    self.session_parses += 1;
                    let Some((base, spare)) = Store::parse_session(file) else {
                        continue;
                    };
                    self.session_json.insert(
                        file.clone(),
                        ParsedFile {
                            stamp,
                            base: base.clone(),
                            spare,
                        },
                    );
                    (base, spare)
                }
            };
            // Liveness is checked every refresh: a dead pid drops the row even when the file is unchanged.
            if !self.running(base.pid) || spare {
                continue;
            }
            let mut s = base;
            let url = self.transcript_url(&s.id, &s.cwd, s.since);
            let size = url
                .as_ref()
                .and_then(|u| FileStamp::new(u))
                .map_or(0, |f| f.size);
            if url.is_some() {
                sizes.insert(s.id.clone(), size);
            }
            let (reply, pending) = self.read_tail(&s, url.as_deref(), size);
            s.first_prompt = self.read_head(&s.id, url.as_deref(), size);
            s.ai_title = self.cache.get(&s.id).and_then(|c| c.ai_title.clone());
            s.branch = self.branch_of(&s.cwd);
            s.last_reply = reply;
            s.todo = self.read_todo(&s.id);
            if s.is_permission_prompt() {
                s.pending = pending;
            }
            list.push(s);
        }
        self.session_json.retain(|k, _| seen_files.contains(k));
        // One conversation open in two processes (resumed in another pane): keep only the newest, so rows and caches keyed by id stay unique.
        let mut newest: HashMap<String, (Option<SystemTime>, i32)> = HashMap::new();
        for s in &list {
            if newest.get(&s.id).is_none_or(|n| n.0 <= s.since) {
                newest.insert(s.id.clone(), (s.since, s.pid));
            }
        }
        list.retain(|s| newest.get(&s.id).map(|n| n.1) == Some(s.pid));
        // After the dedupe, so two processes sharing a sessionId cannot reset each other's cached activity.
        for s in &mut list {
            let url = self.transcript_url(&s.id, &s.cwd, s.since);
            let size = sizes.get(&s.id).copied().unwrap_or(0);
            let a = self.read_activity(s, url.as_deref(), size);
            if let Some(MainTurn::Ended { background: n, at }) = a.turn {
                s.background_agents = Some(n);
                if n == 0
                    && let (Some(at), Some(since)) = (at, s.since)
                {
                    s.busy_after_turn = since > at;
                }
                // The first loop skipped the reply read: the row was still `busy` then.
                s.last_reply = self.read_tail(s, url.as_deref(), size).0;
            } else {
                s.activity = a.text;
                s.agents = a.agents;
            }
        }

        if list.is_empty() && self.procs.closed.is_empty() {
            if self.empty_since.is_none() {
                self.empty_since = Some(now);
            }
        } else {
            self.empty_since = None;
        }

        if let Some(feed) = &self.feed_monitor {
            feed.update(
                list.iter().any(|s| {
                    s.status == "busy" || (s.status == "waiting" && !s.is_permission_prompt())
                }),
                now,
            );
            let items = feed.take_snapshot();
            for s in &mut list {
                if let Some(q) = feed::match_session(&items, &s.id) {
                    self.last_question.insert(s.id.clone(), q.clone());
                    s.question = Some(q.clone());
                    s.question_live = true;
                } else if s.status == "waiting"
                    && !s.is_permission_prompt()
                    && let Some(q) = self.last_question.get(&s.id)
                {
                    s.question = Some(q.clone());
                } else {
                    self.last_question.remove(&s.id);
                }
            }
        }
        let mut new_unseen = self.unseen.clone();
        for s in &list {
            let last = self.last_status.get(&s.id).map(String::as_str);
            if last.is_some_and(|l| l != "busy") && s.status == "busy" {
                new_unseen.remove(&s.id);
            }
            let mut announced = false;
            if last == Some("busy") && matches!(s.status.as_str(), "idle" | "shell" | "waiting") {
                new_unseen.entry(s.id.clone()).or_default().claude = true;
                announced = true;
                let request = s.question.as_ref().map(|q| &q.request_id);
                if s.status == "waiting"
                    && s.question_live
                    && self.announced_question.get(&s.id) == request
                {
                    // Already announced when the question appeared.
                } else {
                    let c = if s.status == "waiting" {
                        Chime::NeedsYou
                    } else {
                        Chime::Done
                    };
                    self.transitions.insert(s.id.clone(), c);
                    if s.status == "waiting" {
                        self.chime = Some(Chime::NeedsYou);
                    } else if self.chime.is_none() {
                        self.chime = Some(Chime::Done);
                    }
                }
            }
            // A question seen for the first time counts as the session starting to wait; one there at first sight stays quiet.
            if s.question_live
                && let Some(q) = &s.question
                && self.announced_question.get(&s.id) != Some(&q.request_id)
            {
                if last == Some("busy") && !announced {
                    new_unseen.entry(s.id.clone()).or_default().claude = true;
                    self.transitions.insert(s.id.clone(), Chime::NeedsYou);
                    self.chime = Some(Chime::NeedsYou);
                }
                self.announced_question
                    .insert(s.id.clone(), q.request_id.clone());
            }
            self.last_status.insert(s.id.clone(), s.status.clone());
        }
        let targets: Vec<ScanTarget> = list
            .iter()
            .map(|s| ScanTarget {
                session_id: s.id.clone(),
                transcript: self.urls.get(&s.id).cloned(),
                size: sizes.get(&s.id).copied(),
            })
            .collect();
        self.scanner.update(targets, now);
        let scans = self.scanner.take_snapshot();
        let mut learned = self.jira_projects.clone();
        for r in scans.values() {
            for (prefix, host) in &r.jira_hosts {
                if jira::replaces(learned.get(prefix).map(String::as_str), host) {
                    learned.insert(prefix.clone(), host.clone());
                }
            }
        }
        if learned != self.jira_projects {
            self.jira_projects = learned;
            self.projects_generation += 1;
        }
        let empty_scan = ScanResult::default();
        let mut entries: HashMap<String, Vec<ButtonEntry>> = HashMap::new();
        for s in &list {
            let key = ButtonKey {
                version: scans.get(&s.id).map_or(0, |r| r.version),
                projects_generation: self.projects_generation,
            };
            if self.button_cache.get(&s.id).map(|c| &c.0) != Some(&key) {
                let b = buttons(scans.get(&s.id).unwrap_or(&empty_scan), &self.jira_projects);
                self.button_cache.insert(s.id.clone(), (key, b));
            }
            entries.insert(s.id.clone(), self.button_cache[&s.id].1.clone());
        }
        let mut issues: HashMap<String, JiraIssue> = HashMap::new();
        let mut jira_state: Option<ConnectionState> = None;
        if let Some(jm) = &self.jira_monitor {
            // Only `*.atlassian.net` hosts get the token; the monitor drops duplicates.
            let targets: Vec<JiraTarget> = list
                .iter()
                .flat_map(|s| entries[&s.id].iter())
                .filter_map(|e| e.target.clone())
                .filter(|t| jira::is_atlassian_host(&t.host))
                .collect();
            jm.update(&targets, now);
            let snap = jm.take_snapshot();
            issues = snap.issues;
            // Icons are kept once per URL: the first bytes win.
            for (url, d) in snap.icons {
                self.issue_icons.entry(url).or_insert(d);
            }
            jira_state = Some(snap.connection);
        }
        // Entries of a PR the user hid in this session are gone from links and the menu.
        for (id, bs) in entries.iter_mut() {
            if let Some(hidden) = self.hidden_prs.get(id) {
                bs.retain(|b| !b.refs.iter().any(|r| hidden.contains(r)));
            }
        }
        let mut pr_state: Option<ConnectionState> = None;
        let mut all_prs: HashMap<String, Vec<PRStatus>> = HashMap::new();
        for s in &mut list {
            s.links = entries[&s.id]
                .iter()
                .map(|b| {
                    let mut link = b.link.clone();
                    if let Some(t) = &b.target {
                        let issue = issues.get(&t.key);
                        link.help = help(&t.key, issue);
                        link.icon_url = issue.and_then(|i| i.icon_url.clone());
                        link.tone = issue.and_then(|i| i.tone).map(ItemTone::from);
                    }
                    link
                })
                .collect();
        }
        if let Some(pm) = &self.pr_monitor {
            let targets: Vec<PRTarget> = list
                .iter()
                .map(|s| PRTarget {
                    session_id: s.id.clone(),
                    link_refs: scans
                        .get(&s.id)
                        .map(|r| r.pr_refs.clone())
                        .unwrap_or_default(),
                    cwd: s.cwd.clone(),
                    branch: s.branch.clone(),
                    hidden_refs: self.hidden_prs.get(&s.id).cloned().unwrap_or_default(),
                })
                .collect();
            pm.update(&targets, now);
            let snap = pm.take_snapshot();
            pr_state = Some(snap.connection);
            for s in &mut list {
                let mut all = snap.prs.get(&s.id).cloned().unwrap_or_default();
                if let Some(hidden) = self.hidden_prs.get(&s.id) {
                    all.retain(|p| !hidden.contains(&p.ref_));
                }
                let events: Vec<PRRef> = all
                    .iter()
                    .map(|p| p.ref_.clone())
                    .filter(|r| snap.event_refs.contains(r))
                    .collect();
                if !events.is_empty() {
                    new_unseen
                        .entry(s.id.clone())
                        .or_default()
                        .prs
                        .extend(events);
                }
                if let Some(mark) = new_unseen.get_mut(&s.id) {
                    let refs: HashSet<&PRRef> = all.iter().map(|p| &p.ref_).collect();
                    mark.prs.retain(|r| refs.contains(r));
                    if !mark.claude && mark.prs.is_empty() {
                        new_unseen.remove(&s.id);
                    }
                }
                s.prs = all.iter().take(PR_LIST_MAX).cloned().collect();
                let shown: HashSet<&PRRef> = s.prs.iter().map(|p| &p.ref_).collect();
                if !shown.is_empty() {
                    let refs = &entries[&s.id];
                    s.links = std::mem::take(&mut s.links)
                        .into_iter()
                        .enumerate()
                        .filter(|(i, _)| !refs[*i].refs.iter().any(|r| shown.contains(r)))
                        .map(|(_, l)| l)
                        .collect();
                }
                self.tracked_prs
                    .insert(s.id.clone(), all.iter().map(|p| p.ref_.clone()).collect());
                all_prs.insert(s.id.clone(), all);
            }
            if self.pr_last_success != snap.last_success {
                self.pr_last_success = snap.last_success;
            }
            if !snap.events.is_empty() {
                self.peek_until = Some(add_secs(now, PEEK_FOR));
                self.pr_chime = pull_requests::chime(&snap.events);
                if let Some(c) = self.pr_chime
                    && self.chime != Some(Chime::NeedsYou)
                {
                    self.chime = Some(c);
                }
            }
        }
        let has_pr_monitor = self.pr_monitor.is_some();
        if !has_pr_monitor {
            self.tracked_prs.clear();
        }
        for s in &mut list {
            let e = &entries[&s.id];
            let prs = has_pr_monitor.then(|| all_prs.get(&s.id).map_or(&[][..], Vec::as_slice));
            s.menu = menu(e, &issues, prs);
            s.menu_needs_jira = jira_state != Some(ConnectionState::Ok)
                && e.iter().any(|b| {
                    b.target
                        .as_ref()
                        .is_some_and(|t| jira::is_atlassian_host(&t.host))
                });
        }
        let lines = HashMap::from([
            (
                Service::Bitbucket,
                connection_line(
                    has_pr_monitor,
                    self.accounts.get(&Service::Bitbucket),
                    pr_state.unwrap_or(ConnectionState::Checking),
                ),
            ),
            (
                Service::Jira,
                if self.jira_host().is_none() {
                    ConnectionLine::NoSite
                } else {
                    connection_line(
                        self.jira_monitor.is_some(),
                        self.accounts.get(&Service::Jira),
                        jira_state.unwrap_or(ConnectionState::Checking),
                    )
                },
            ),
        ]);
        if lines != self.connections {
            self.connections = lines;
        }
        let live: HashSet<String> = list.iter().map(|s| s.id.clone()).collect();
        let mut now_closed = self.closed.clone();
        // `sessions` still holds the previous refresh. A session that left is closed only once its process is gone: `/clear` gives the same pid a new id.
        let live_pids: HashSet<i32> = list.iter().map(|s| s.pid).collect();
        // Claude deletes the session file while the process is still exiting (or a session leaves the list while its process lives):
        // keep it in `departed` for a few seconds and close it once the process is gone.
        for s in &self.sessions {
            if !live.contains(&s.id)
                && !live_pids.contains(&s.pid)
                && !self.departed.contains_key(&s.id)
            {
                self.departed
                    .insert(s.id.clone(), (s.closed(now), s.pid, now));
            }
        }
        let ids: Vec<String> = self.departed.keys().cloned().collect();
        for id in ids {
            let (session, pid, since) = self.departed[&id].clone();
            if live.contains(&id)
                || live_pids.contains(&pid)
                || secs_since(now, since) > DEPARTED_WINDOW
            {
                self.departed.remove(&id);
            } else if !self.running(pid) {
                now_closed.add(session);
                self.departed.remove(&id);
            }
        }
        now_closed.revive(&live);
        let keep: HashSet<String> = self
            .procs
            .closed
            .iter()
            .map(|p| p.session.clone())
            .collect();
        if self.procs_seen {
            now_closed.prune(now, &keep);
        }
        if now_closed != self.closed {
            if let Some(store) = &self.closed_store {
                store.save(&now_closed);
            }
            self.closed = now_closed;
        }
        self.last_status.retain(|k, _| live.contains(k));
        self.last_question.retain(|k, _| live.contains(k));
        self.announced_question.retain(|k, _| live.contains(k));
        let asking = |id: &String| list.iter().any(|s| &s.id == id && is_busy_or_waiting(s));
        self.sent.retain(|k, _| asking(k));
        self.failed.retain(|k, _| asking(k));
        self.cache.retain(|k, _| live.contains(k));
        self.button_cache.retain(|k, _| live.contains(k));
        self.urls.retain(|k, _| live.contains(k));
        self.url_tried.retain(|k, _| live.contains(k));
        self.heads.retain(|k, _| live.contains(k));
        self.todos.retain(|k, _| live.contains(k));
        if let Branches::Git(g) = &mut self.branches {
            g.prune(&list.iter().map(|s| s.cwd.clone()).collect());
        }
        self.activities.retain(|k, _| live.contains(k));
        for s in &list {
            if self.pids.get(&s.id).is_some_and(|p| *p != s.pid) {
                self.surfaces.remove(&s.id);
                self.surface_misses.remove(&s.id);
            }
        }
        self.surface_misses.retain(|k| live.contains(k));
        self.hidden_prs.retain(|k, _| live.contains(k));
        self.tracked_prs.retain(|k, _| live.contains(k));
        new_unseen.retain(|k, _| live.contains(k));
        if new_unseen != self.unseen {
            self.unseen = new_unseen;
        }
        self.pids = list.iter().map(|s| (s.id.clone(), s.pid)).collect();
        self.surfaces.retain(|k, _| live.contains(k));
        self.drafts
            .retain(|k, _| live.contains(k.split('#').next().unwrap_or("")));
        let open_tools: HashSet<&str> = list
            .iter()
            .filter_map(|s| s.approvable().map(|p| p.tool_id.as_str()))
            .collect();
        self.shown_commands
            .retain(|t| open_tools.contains(t.as_str()));

        for s in &mut list {
            s.block = block_of(s, &self.sent, &self.failed, now);
        }
        let rank = |s: &Session| match s.color_status() {
            "waiting" => 0,
            "idle" => 1,
            "busy" => 2,
            _ => 3,
        };
        list.sort_by(|a, b| (rank(a), a.title()).cmp(&(rank(b), b.title())));
        if list != self.sessions {
            self.sessions = list;
        }
    }
}

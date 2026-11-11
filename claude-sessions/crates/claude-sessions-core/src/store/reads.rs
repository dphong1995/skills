//! The per-session file reads of a refresh: session JSON, transcript head and tail, running-tool line, task list.

use super::*;
use crate::time::checked_from_unix_secs;

impl Store {
    pub(super) fn running(&self, pid: i32) -> bool {
        (self.is_running)(pid)
    }

    pub(super) fn transcript_url(
        &mut self,
        id: &str,
        cwd: &str,
        since: Option<SystemTime>,
    ) -> Option<PathBuf> {
        if let Some(u) = self.urls.get(id) {
            return Some(u.clone());
        }
        if let Some(tried) = self.url_tried.get(id)
            && *tried == since
        {
            return None;
        }
        self.url_tried.insert(id.to_string(), since);
        let u = transcript::find(id, cwd, &self.projects)?;
        self.urls.insert(id.to_string(), u.clone());
        Some(u)
    }

    pub(super) fn read_todo(&mut self, id: &str) -> Option<Todo> {
        let d = self.tasks.join(id);
        let dir_stamp = FileStamp::new(&d);
        // An unchanged directory has the same files: re-stat them (an in-place rewrite does not touch the directory) instead of listing.
        let files = match self.todos.get(id) {
            Some(c) if c.key.dir == dir_stamp => c.files.clone(),
            _ => {
                self.task_listings += 1;
                tasks::files(&d)
            }
        };
        let key = TaskKey {
            dir: dir_stamp,
            stamps: files.iter().map(|f| FileStamp::new(f)).collect(),
        };
        if let Some(c) = self.todos.get(id)
            && c.key == key
        {
            return c.todo.clone();
        }
        let todo = tasks::read_files(&files);
        self.todos.insert(
            id.to_string(),
            TodoCache {
                key,
                files,
                todo: todo.clone(),
            },
        );
        todo
    }

    /// The head of an append-only transcript cannot change once it holds a prompt or the file outgrew the head window.
    pub(super) fn head_is_final(&self, id: &str) -> bool {
        self.heads
            .get(id)
            .is_some_and(|h| h.prompt.is_some() || h.size > transcript::HEAD_BYTES as u64)
    }

    pub(super) fn read_head(&mut self, id: &str, url: Option<&Path>, size: u64) -> Option<String> {
        let url = url?;
        if let Some(h) = self.heads.get(id)
            && (self.head_is_final(id) || h.size == size)
        {
            return h.prompt.clone();
        }
        let d = transcript::head(url, transcript::HEAD_BYTES);
        self.transcript_bytes_read += d.len();
        let prompt = transcript::first_prompt(&d).map(|f| transcript::display_text(&f, 200));
        self.heads.insert(
            id.to_string(),
            Head {
                prompt: prompt.clone(),
                size,
            },
        );
        prompt
    }

    pub(super) fn read_tail(
        &mut self,
        s: &Session,
        url: Option<&Path>,
        size: u64,
    ) -> (Option<String>, Option<PendingState>) {
        let Some(url) = url.filter(|_| s.is_finished()) else {
            return (None, None);
        };
        let size = size as i64;
        let c = self.cache.entry(s.id.clone()).or_insert_with(|| Cached {
            since: s.since,
            size: -1,
            ..Cached::default()
        });
        let prompt = s.is_permission_prompt();
        if c.size == size && c.since == s.since && c.prompt == prompt {
            return (c.reply.clone(), c.pending.clone());
        }
        c.since = s.since;
        c.size = size;
        c.prompt = prompt;
        let tail = transcript::tail(url, TAIL_BYTES);
        self.transcript_bytes_read += tail.len();
        let c = self.cache.get_mut(&s.id).expect("inserted above");
        c.reply = transcript::last_reply(&tail).map(|f| transcript::display_text(&f, 600));
        c.pending = if prompt {
            transcript::pending(&tail)
        } else {
            None
        };
        if let Some(t) = transcript::last_ai_title(&tail) {
            c.ai_title = Some(t);
        }
        (c.reply.clone(), c.pending.clone())
    }

    /// The last tool call of a busy session, read from the bytes appended since the last refresh (at most the activity window).
    pub(super) fn read_activity(
        &mut self,
        s: &Session,
        url: Option<&Path>,
        size: u64,
    ) -> ActivityRead {
        let cached = self.activities.get(&s.id).cloned();
        let Some(url) = url.filter(|_| s.status == "busy") else {
            if let Some(c) = cached {
                self.activities.insert(
                    s.id.clone(),
                    Activity {
                        offset: c.offset,
                        text: None,
                        turn: None,
                        agents: Vec::new(),
                    },
                );
            }
            return ActivityRead {
                text: None,
                turn: None,
                agents: Vec::new(),
            };
        };
        let window = transcript::ACTIVITY_WINDOW;
        let tail_start = size.saturating_sub(window);
        let from = match &cached {
            Some(c) if size >= c.offset => {
                if size == c.offset {
                    return ActivityRead {
                        text: c.text.clone(),
                        turn: c.turn.clone(),
                        agents: c.agents.iter().map(|a| a.1.clone()).collect(),
                    };
                }
                c.offset.max(tail_start)
            }
            _ => tail_start,
        };
        let continuing = Some(from) == cached.as_ref().map(|c| c.offset);
        let at_line_start = from == 0 || continuing;
        self.activity_reads += 1;
        let d = transcript::read(url, from, size);
        self.transcript_bytes_read += d.len();
        let mut body: &[u8] = &d;
        let mut offset = from;
        if !at_line_start {
            let Some(nl) = d.iter().position(|b| *b == 0x0A) else {
                let kept = if continuing {
                    cached
                        .as_ref()
                        .map(|c| c.agents.clone())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let text = cached.as_ref().and_then(|c| c.text.clone());
                let turn = cached.as_ref().and_then(|c| c.turn.clone());
                self.activities.insert(
                    s.id.clone(),
                    Activity {
                        offset: size,
                        text: text.clone(),
                        turn: turn.clone(),
                        agents: kept.clone(),
                    },
                );
                return ActivityRead {
                    text,
                    turn,
                    agents: kept.into_iter().map(|a| a.1).collect(),
                };
            };
            body = &d[nl + 1..];
            offset = from + nl as u64 + 1;
        }
        let mut text = cached.as_ref().and_then(|c| c.text.clone());
        let mut turn = cached.as_ref().and_then(|c| c.turn.clone());
        let mut agents = if continuing {
            cached
                .as_ref()
                .map(|c| c.agents.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(end) = body.iter().rposition(|b| *b == 0x0A) {
            offset += end as u64 + 1;
            let lines = &body[..=end];
            text = transcript::last_tool_use(lines).or(text);
            turn = transcript::main_turn(lines).or(turn);
            let open: Vec<String> = agents.iter().map(|a| a.0.clone()).collect();
            let changes = transcript::agent_changes(lines, &open);
            agents.extend(changes.opened);
            agents.retain(|a| !changes.closed.contains(&a.0));
        }
        if matches!(turn, Some(MainTurn::Ended { .. })) {
            text = None;
            agents.clear();
        }
        self.activities.insert(
            s.id.clone(),
            Activity {
                offset,
                text: text.clone(),
                turn: turn.clone(),
                agents: agents.clone(),
            },
        );
        ActivityRead {
            text,
            turn,
            agents: agents.into_iter().map(|a| a.1).collect(),
        }
    }

    pub(super) fn branch_of(&mut self, cwd: &str) -> Option<String> {
        match &mut self.branches {
            Branches::Git(g) => g.current(cwd),
            Branches::Custom(f) => f(cwd),
        }
    }

    /// Parses one session file; `None` when it is unreadable or has no pid.
    pub(super) fn parse_session(file: &Path) -> Option<(Session, bool)> {
        let data = std::fs::read(file).ok()?;
        let o: serde_json::Value = serde_json::from_slice(&data).ok()?;
        let o = o.as_object()?;
        let pid = o
            .get("pid")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))?
            as i32;
        let text = |k: &str| o.get(k).and_then(|v| v.as_str()).map(String::from);
        let mut s = Session::new(
            &text("sessionId").unwrap_or_else(|| pid.to_string()),
            &text("name").unwrap_or_else(|| format!("pid {pid}")),
            &text("cwd").unwrap_or_default(),
            &text("status").unwrap_or_default(),
        );
        s.waiting_for = text("waitingFor");
        s.since = o
            .get("statusUpdatedAt")
            .and_then(|v| v.as_f64())
            .and_then(|ms| checked_from_unix_secs(ms / 1000.0));
        s.pid = pid;
        s.name_source = text("nameSource");
        // Skipped like a spare: `claude -p` / SDK runs (entrypoint `sdk-*`) are not sessions the user opened.
        let spare = o.get("spare").and_then(|v| v.as_bool()) == Some(true)
            || text("entrypoint").is_some_and(|e| e.starts_with("sdk-"));
        Some((s, spare))
    }
}

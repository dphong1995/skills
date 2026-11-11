//! What the native text fields mean: typed text is kept as a draft (reply, deny reason) or in the panel (`Khác…`), and Enter in
//! a field does what clicking its send button does. Field keys are made by the views: `reply-<id>`, `deny-<id>`,
//! `other-<id>#<question index>`.

use crate::hits::Target;
use claude_sessions_core::feed;
use claude_sessions_core::panel_state::{EditKind, PanelState};
use claude_sessions_core::store::{Snapshot, Store};
use std::collections::HashMap;

enum Field<'a> {
    Reply(&'a str),
    Deny(&'a str),
    Other(&'a str, usize),
}

fn parse(key: &str) -> Option<Field<'_>> {
    if let Some(id) = key.strip_prefix("reply-") {
        return Some(Field::Reply(id));
    }
    if let Some(id) = key.strip_prefix("deny-") {
        return Some(Field::Deny(id));
    }
    let (id, i) = key.strip_prefix("other-")?.rsplit_once('#')?;
    Some(Field::Other(id, i.parse().ok()?))
}

/// The draft key of a reply or deny field.
fn draft_key(field: &Field) -> Option<String> {
    match field {
        Field::Reply(id) => Some(Store::draft_key(id, EditKind::Reply)),
        Field::Deny(id) => Some(Store::draft_key(id, EditKind::Deny)),
        Field::Other(..) => None,
    }
}

/// The user typed `text` into the field `key`.
pub fn changed(
    key: &str,
    text: &str,
    drafts: &mut HashMap<String, String>,
    panel: &mut PanelState,
    snap: &Snapshot,
) {
    let Some(field) = parse(key) else { return };
    if let Some(draft) = draft_key(&field) {
        drafts.insert(draft, text.to_string());
    } else if let Field::Other(id, i) = field
        && let Some(q) = snap
            .sessions
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.question.as_ref())
    {
        panel.set_other(q, i, text);
    }
}

/// The send button the field `key` belongs to, for Enter in it; `None` when sending would do nothing (empty reply, no open
/// approval, incomplete answers).
pub fn submit_target(
    key: &str,
    drafts: &HashMap<String, String>,
    panel: &PanelState,
    snap: &Snapshot,
) -> Option<Target> {
    let field = parse(key)?;
    let session = |id: &str| snap.sessions.iter().find(|s| s.id == id);
    match field {
        Field::Reply(id) => {
            let typed = drafts.get(&draft_key(&field)?)?;
            (!typed.trim().is_empty()).then(|| Target::ReplySend(id.to_string()))
        }
        Field::Deny(id) => {
            let p = session(id)?.approvable()?;
            Some(Target::DenySend {
                session: id.to_string(),
                tool_id: p.tool_id.clone(),
            })
        }
        Field::Other(id, _) => {
            let q = session(id)?.question.as_ref()?;
            let answers = panel
                .answers
                .get(&q.request_id)
                .cloned()
                .unwrap_or_default();
            let others = panel.others.get(&q.request_id).cloned().unwrap_or_default();
            feed::complete(q, &answers, &others).then(|| Target::Answer(id.to_string()))
        }
    }
}

/// Esc in the field `key`: ends that field's editing (`PanelState.editing`, or the open `Khác…` field), nothing else.
/// Returns whether anything changed.
pub fn cancel(key: &str, panel: &mut PanelState) -> bool {
    let Some(field) = parse(key) else {
        return false;
    };
    let (id, kind) = match field {
        Field::Reply(id) => (id, EditKind::Reply),
        Field::Deny(id) => (id, EditKind::Deny),
        Field::Other(..) => {
            return panel
                .other_key
                .take_if(|k| k == key.strip_prefix("other-").unwrap_or(""))
                .is_some();
        }
    };
    panel
        .editing
        .take_if(|e| e.id == id && e.kind == kind)
        .is_some()
}

/// The text of the field a click on `target` belongs to: the reply or deny-reason draft, empty for other targets.
pub fn text_for<'a>(target: &Target, drafts: &'a HashMap<String, String>) -> &'a str {
    let key = match target {
        Target::ReplySend(id) => Store::draft_key(id, EditKind::Reply),
        Target::DenySend { session, .. } => Store::draft_key(session, EditKind::Deny),
        _ => return "",
    };
    drafts.get(&key).map_or("", String::as_str)
}

/// The one place that puts the drafts into a built scene's fields, so no view reads them.
pub fn fill(scene: &mut claude_sessions_core::scene::Scene, drafts: &HashMap<String, String>) {
    for field in scene.nodes.iter_mut().flat_map(|n| n.fields.iter_mut()) {
        if let Some(draft) = parse(&field.key).as_ref().and_then(draft_key) {
            field.text = drafts.get(&draft).cloned().unwrap_or_default();
        }
    }
}

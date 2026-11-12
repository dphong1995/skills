//! The state and rules of the Settings window, without AppKit, the Keychain or the network: `apply` turns an action into
//! changes of the model and the store plus the effects the app runs. The token is never here: the model knows only whether the
//! token field holds text, and the app adds the text to a check or save from the native field.

use std::collections::HashMap;

use crate::hotkey::Which;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::pull_requests::DEFAULT_PROMPT;
use claude_sessions_core::scene::ThemeColor;
use claude_sessions_core::store::{ConnectionLine, Store};
use claude_sessions_core::url_guard::{self, TokenHost};
use url::Url;

pub const PROMPT_KEY: &str = "settings.prompt";
pub const SITE_KEY: &str = "settings.site";

const EMAIL_PREFIX: &str = "settings.email.";
const TOKEN_PREFIX: &str = "settings.token.";

const NO_KEYCHAIN: &str = "✗ Bản chạy thử không dùng Keychain";
const BAD_SITE: &str = "✗ Site phải có dạng <tên>.atlassian.net";

fn slug(s: Service) -> &'static str {
    match s {
        Service::Jira => "jira",
        Service::Bitbucket => "bitbucket",
    }
}

fn service_of(slug_text: &str) -> Option<Service> {
    match slug_text {
        "jira" => Some(Service::Jira),
        "bitbucket" => Some(Service::Bitbucket),
        _ => None,
    }
}

pub fn email_key(s: Service) -> String {
    format!("{EMAIL_PREFIX}{}", slug(s))
}

/// The key of a token field; `generation` changes after a save so that the native field is made again, empty.
pub fn token_key(s: Service, generation: u32) -> String {
    format!("{TOKEN_PREFIX}{}.{generation}", slug(s))
}

/// The service whose token field has `key`.
pub fn token_service(key: &str) -> Option<Service> {
    let rest = key.strip_prefix(TOKEN_PREFIX)?;
    service_of(rest.split('.').next()?)
}

/// What a typing change of the field `key` means.
pub fn changed_action(key: &str, text: &str) -> Option<SettingsAction> {
    if key == PROMPT_KEY {
        return Some(SettingsAction::PromptChanged(text.to_string()));
    }
    if key == SITE_KEY {
        return Some(SettingsAction::SiteChanged(text.to_string()));
    }
    if let Some(s) = key.strip_prefix(EMAIL_PREFIX).and_then(service_of) {
        return Some(SettingsAction::EmailChanged(s, text.to_string()));
    }
    token_service(key).map(|s| SettingsAction::TokenEdited(s, !text.is_empty()))
}

/// Keeps the typed token text in `tokens` (removed when the field is empty) and returns what the change means; the text
/// of any other field is not kept.
pub fn note_field(
    tokens: &mut HashMap<Service, String>,
    key: &str,
    text: &str,
) -> Option<SettingsAction> {
    if let Some(service) = token_service(key) {
        if text.is_empty() {
            tokens.remove(&service);
        } else {
            tokens.insert(service, text.to_string());
        }
    }
    changed_action(key, text)
}

/// What leaving or submitting the field `key` means.
pub fn submit_action(key: &str) -> Option<SettingsAction> {
    match key {
        PROMPT_KEY => Some(SettingsAction::PromptCommit),
        SITE_KEY => Some(SettingsAction::SiteCommit),
        _ => None,
    }
}

/// The host of a typed site, lower case: a pasted URL loses its scheme, path, query and port; `None` when empty. Text that
/// does not parse is returned trimmed, so the host guard refuses it.
pub fn normalize_site(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let url = if t.contains("://") {
        t.to_string()
    } else {
        format!("https://{t}")
    };
    let host = Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string));
    Some(host.unwrap_or_else(|| t.to_string()))
}

/// Whether a normalised site may receive the Jira token: the same guard the requests use.
fn site_ok(site: &str) -> bool {
    url_guard::token_url(site, "/", TokenHost::Jira).is_some()
}

/// The prompt to save for typed `text`: trimmed, the default when empty.
pub fn prompt_value(text: &str, default: &str) -> String {
    let t = text.trim();
    if t.is_empty() { default } else { t }.to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub enum CheckState {
    Idle,
    Checking,
    Saving,
    Ok(String),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    Check,
    Save,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceForm {
    pub email: String,
    /// The token field holds text (never the text itself).
    pub token_typed: bool,
    /// Part of the token field's key; bumped to empty the field.
    pub token_gen: u32,
    pub check: CheckState,
    /// The account a check or save of this window confirmed.
    pub verified: Option<String>,
}

impl ServiceForm {
    fn new(email: String) -> Self {
        ServiceForm {
            email,
            token_typed: false,
            token_gen: 0,
            check: CheckState::Idle,
            verified: None,
        }
    }

    fn busy(&self) -> bool {
        matches!(self.check, CheckState::Checking | CheckState::Saving)
    }

    fn edited(&mut self) {
        if !self.busy() {
            self.check = CheckState::Idle;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsModel {
    /// False under a fixture or `CLAUDE_SESSIONS_ROOT`: the Keychain is not used.
    pub live: bool,
    pub prompt: String,
    /// The prompt as loaded or last saved; a commit that does not change it writes nothing.
    pub saved_prompt: String,
    pub site: String,
    pub jira: ServiceForm,
    pub bitbucket: ServiceForm,
}

impl SettingsModel {
    pub fn new(store: &Store, pr_prompt: &str, live: bool) -> Self {
        let email = |s: Service| store.accounts.get(&s).cloned().unwrap_or_default();
        SettingsModel {
            live,
            prompt: pr_prompt.to_string(),
            saved_prompt: pr_prompt.to_string(),
            site: store.jira_site.clone().unwrap_or_default(),
            jira: ServiceForm::new(email(Service::Jira)),
            bitbucket: ServiceForm::new(email(Service::Bitbucket)),
        }
    }

    pub fn form(&self, s: Service) -> &ServiceForm {
        match s {
            Service::Jira => &self.jira,
            Service::Bitbucket => &self.bitbucket,
        }
    }

    pub fn form_mut(&mut self, s: Service) -> &mut ServiceForm {
        match s {
            Service::Jira => &mut self.jira,
            Service::Bitbucket => &mut self.bitbucket,
        }
    }

    /// The Jira site a check would call: the typed one, else the guess.
    pub fn effective_site(&self, store: &Store) -> Option<String> {
        normalize_site(&self.site).or_else(|| store.jira_guess())
    }

    /// A job of this service started before the window opened and is still running: show it and take no second one.
    pub fn resume(&mut self, service: Service, job: Job) {
        self.form_mut(service).check = match job {
            Job::Check => CheckState::Checking,
            Job::Save => CheckState::Saving,
        };
    }

    /// Records the end of a check or save; `Ok` carries the account.
    pub fn done(&mut self, service: Service, job: Job, result: Result<String, String>) {
        let form = self.form_mut(service);
        match (job, result) {
            (Job::Check, Ok(account)) => {
                form.check = CheckState::Ok(format!("✓ Token dùng được · {account}"));
                form.verified = Some(account);
            }
            (Job::Save, Ok(account)) => {
                form.check = CheckState::Ok("✓ Đã lưu vào Keychain".to_string());
                form.verified = Some(account.clone());
                form.email = account;
                form.token_typed = false;
                form.token_gen += 1;
            }
            (_, Err(text)) => form.check = CheckState::Failed(text),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsAction {
    SetOpacity(f64),
    ToggleMute,
    PickHotkey(Which),
    PromptChanged(String),
    PromptCommit,
    PromptReset,
    SiteChanged(String),
    SiteCommit,
    EmailChanged(Service, String),
    /// The token field changed; only whether it holds text.
    TokenEdited(Service, bool),
    Check(Service),
    Save(Service),
    OpenTokenPage,
    Close,
}

/// What the app does after an action; none carries the token.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsEffect {
    Persist,
    SetPrompt(String),
    PopupHotkey(Which),
    RunCheck {
        service: Service,
        email: String,
        site: Option<String>,
    },
    RunSave {
        service: Service,
        email: String,
        site: Option<String>,
    },
    OpenTokenPage,
    CloseWindow,
}

pub fn apply(
    model: &mut SettingsModel,
    store: &mut Store,
    action: SettingsAction,
) -> Vec<SettingsEffect> {
    use SettingsAction as A;
    match action {
        A::SetOpacity(v) => {
            store.bg_opacity = v;
            vec![SettingsEffect::Persist]
        }
        A::ToggleMute => {
            store.muted = !store.muted;
            vec![SettingsEffect::Persist]
        }
        A::PickHotkey(w) => vec![SettingsEffect::PopupHotkey(w)],
        A::OpenTokenPage => vec![SettingsEffect::OpenTokenPage],
        A::PromptChanged(t) => {
            model.prompt = t;
            vec![]
        }
        A::PromptCommit => {
            model.prompt = prompt_value(&model.prompt, DEFAULT_PROMPT);
            if model.prompt == prompt_value(&model.saved_prompt, DEFAULT_PROMPT) {
                return vec![];
            }
            model.saved_prompt = model.prompt.clone();
            vec![SettingsEffect::SetPrompt(model.prompt.clone())]
        }
        A::PromptReset => {
            model.prompt = DEFAULT_PROMPT.to_string();
            model.saved_prompt = model.prompt.clone();
            vec![SettingsEffect::SetPrompt(DEFAULT_PROMPT.to_string())]
        }
        A::SiteChanged(t) => {
            model.site = t;
            model.jira.edited();
            vec![]
        }
        A::SiteCommit => {
            let site = normalize_site(&model.site);
            if site.as_deref().is_some_and(|s| !site_ok(s)) {
                if !model.jira.busy() {
                    model.jira.check = CheckState::Failed(BAD_SITE.to_string());
                }
                vec![]
            } else if site == store.jira_site {
                vec![]
            } else {
                store.jira_site = site;
                vec![SettingsEffect::Persist]
            }
        }
        A::EmailChanged(s, t) => {
            let form = model.form_mut(s);
            form.email = t;
            form.edited();
            vec![]
        }
        A::TokenEdited(s, typed) => {
            let form = model.form_mut(s);
            form.token_typed = typed;
            form.edited();
            vec![]
        }
        A::Check(s) => start(model, store, s, Job::Check),
        A::Save(s) => start(model, store, s, Job::Save),
        A::Close => {
            let mut effects = apply(model, store, A::PromptCommit);
            effects.extend(apply(model, store, A::SiteCommit));
            effects.push(SettingsEffect::CloseWindow);
            effects
        }
    }
}

fn start(
    model: &mut SettingsModel,
    store: &Store,
    service: Service,
    job: Job,
) -> Vec<SettingsEffect> {
    if model.form(service).busy() {
        return vec![];
    }
    let refuse = |model: &mut SettingsModel, text: &str| {
        model.form_mut(service).check = CheckState::Failed(text.to_string());
        vec![]
    };
    if !model.live {
        return refuse(model, NO_KEYCHAIN);
    }
    if job == Job::Save && !model.form(service).token_typed {
        return refuse(model, "✗ Nhập token mới để lưu");
    }
    let site = if service == Service::Jira {
        let Some(site) = model.effective_site(store) else {
            return refuse(model, "✗ Chưa có site Jira");
        };
        if !site_ok(&site) {
            return refuse(model, BAD_SITE);
        }
        Some(site)
    } else {
        None
    };
    let email = model.form(service).email.trim().to_string();
    let (state, effect) = match job {
        Job::Check => (
            CheckState::Checking,
            SettingsEffect::RunCheck {
                service,
                email,
                site,
            },
        ),
        Job::Save => (
            CheckState::Saving,
            SettingsEffect::RunSave {
                service,
                email,
                site,
            },
        ),
    };
    model.form_mut(service).check = state;
    vec![effect]
}

/// The status text of a service's header and its colour: the monitor's line without `<name>: `, or the account this window
/// just confirmed while the monitor is still checking.
pub fn status_line(model: &SettingsModel, store: &Store, service: Service) -> (String, ThemeColor) {
    let line = store.connection(service);
    if let (ConnectionLine::Checking(_), Some(account)) = (line, &model.form(service).verified) {
        return (format!("✓ {account}"), ThemeColor::Green);
    }
    let color = match line {
        ConnectionLine::Ok(_) => ThemeColor::Green,
        ConnectionLine::Checking(_) => ThemeColor::TextSecondary,
        _ => ThemeColor::Red,
    };
    (store.connection_status(service), color)
}

/// What the Settings window shows that the monitors, the app and the hotkey registration change on their own: the connection
/// lines, the accounts and the failed hotkeys. The tick re-renders the window only when this differs from the last render's.
pub fn external_key(store: &Store, failed: &std::collections::BTreeSet<&'static str>) -> String {
    let mut key = String::new();
    for s in Service::ALL {
        key.push_str(&format!(
            "{:?}|{:?}|{:?};",
            s,
            store.connection(s),
            store.accounts.get(&s)
        ));
    }
    key.push_str(&format!("{failed:?}"));
    key
}

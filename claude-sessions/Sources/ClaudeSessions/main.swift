import AppKit
import ClaudeSessionsKit
import Combine
import ServiceManagement

final class Panel {
    let uuid: String
    let window: WidgetPanel
    let state: PanelState
    var screen: NSRect = .zero
    var cancellables: Set<AnyCancellable> = []

    init(uuid: String, window: WidgetPanel, state: PanelState) {
        self.uuid = uuid
        self.window = window
        self.state = state
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Absolute dir holding `sessions/`, `projects/` and `tasks/`; set by `perf.sh` to run on a fixture.
    static let root = ProcessInfo.processInfo.environment["CLAUDE_SESSIONS_ROOT"].map { URL(fileURLWithPath: $0, isDirectory: true) }
    let store = AppDelegate.root.map {
        Store(dir: $0.appendingPathComponent("sessions"), projects: $0.appendingPathComponent("projects"),
              tasks: $0.appendingPathComponent("tasks"), startHidden: true)
    } ?? Store(startHidden: true, quota: QuotaReader(url: FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".claude/plugins/oh-my-claudecode/.usage-cache-anthropic.json")),
                         closedStore: .standard)
    var panels: [String: Panel] = [:]
    let hotKeys = HotKeys()
    var lastNext: (id: String, at: Date)?
    var timer: Timer?
    var monitors: [Any] = []
    var sending: Set<String> = []
    var lookingUp: Set<String> = []
    var tick = 0
    var checkingFocus = false
    /// Bumped whenever the marks' focus streak is reset, so a slow query started before it is ignored.
    var focusGen = 0
    var cancellables: Set<AnyCancellable> = []
    /// Live API clients, kept so a saved token can be picked up with `reload()`. Main thread only.
    var bitbucketClient: BitbucketClient?
    var jiraClient: JiraClient?
    var connecting = false

    func displayUUID(_ s: NSScreen) -> String? {
        guard let id = s.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID,
              let u = CGDisplayCreateUUIDFromDisplayID(id)?.takeRetainedValue(),
              let str = CFUUIDCreateString(nil, u) else { return nil }
        return str as String
    }

    func keyedScreens() -> [(uuid: String, screen: NSScreen)] {
        let keyed = NSScreen.screens.compactMap { s in displayUUID(s).map { (uuid: $0, screen: s) } }
        if keyed.isEmpty, let first = NSScreen.screens.first { return [(uuid: "main", screen: first)] }
        return keyed
    }

    func makePanel(uuid: String) -> Panel {
        let window = WidgetPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = false
        window.level = .statusBar
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        window.hidesOnDeactivate = false
        window.ignoresMouseEvents = true
        window.acceptsMouseMovedEvents = true
        let state = PanelState()
        window.state = state
        let p = Panel(uuid: uuid, window: window, state: state)
        let host = DragHostingView(rootView: WidgetView(store: store, panel: state, onAction: { [weak self] in self?.handle($0) }))
        host.onDragEnd = { [weak self, weak p] start, raw, clamped in
            guard let self, let p else { return }
            settleAfterDrop(p, start: start, raw: raw, clamped: clamped)
        }
        host.clampOrigin = { [weak self, weak p] origin in
            guard let self, let p else { return origin }
            return Layout.clampDrag(windowOrigin: origin, windowSize: p.window.frame.size, visible: Layout.size(store, p.state), anchor: p.state.anchor, screen: p.screen)
        }
        window.contentView = host

        state.$editing.sink { [weak self, weak window] e in
            guard let window else { return }
            // @Published fires before the value is stored, so defer until canBecomeKey sees it.
            DispatchQueue.main.async { [weak self] in
                guard let store = self?.store else { return }
                if e != nil {
                    if !store.hidden { window.makeKey() }
                } else if window.isKeyWindow && !store.hidden {
                    // resignKey() is only a callback; reordering the panel is what hands key status back.
                    window.orderOut(nil)
                    window.orderFrontRegardless()
                }
            }
        }.store(in: &p.cancellables)
        NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification, object: window)
            .sink { [weak state] _ in state?.editing = nil }
            .store(in: &p.cancellables)
        if !store.hidden { window.orderFrontRegardless() }
        return p
    }

    func syncPanels() {
        let keyed = keyedScreens()
        for (uuid, p) in panels where !keyed.contains(where: { $0.uuid == uuid }) {
            p.window.close()
            panels[uuid] = nil
        }

        var origins = UserDefaults.standard.dictionary(forKey: "pillOrigins") as? [String: [Double]] ?? [:]
        for (uuid, screen) in keyed {
            let p = panels[uuid] ?? makePanel(uuid: uuid)
            panels[uuid] = p
            let s = screen.frame
            p.screen = s
            if let o = origins[uuid].flatMap({ Layout.origin(offset: $0, in: s) }) {
                place(p, pillOrigin: o, save: false)
            } else if let ref = keyed.first(where: { origins[$0.uuid] != nil }),
                      let refOrigin = origins[ref.uuid].flatMap({ Layout.origin(offset: $0, in: ref.screen.frame) }) {
                let rf = ref.screen.frame
                let anchor = Anchor.near(Layout.pillCentre(refOrigin), in: rf)
                origins[uuid] = place(p, pillOrigin: Layout.relativeOrigin(refOrigin, from: rf, to: s, anchor: anchor), save: true)
            } else {
                let v = screen.visibleFrame
                origins[uuid] = place(p, pillOrigin: CGPoint(x: v.maxX - Layout.collapsed.width - 16, y: v.maxY - Layout.collapsed.height - 10), save: true)
            }
        }
    }

    /// Moves the pre-`pillOrigins` keys (`pillX`, `pillY`, `pillWidth`) into `pillOrigins` once.
    func migrateLegacyPillOrigin() {
        let d = UserDefaults.standard
        guard d.object(forKey: "pillX") != nil else { return }
        if d.object(forKey: "pillOrigins") == nil {
            let keyed = keyedScreens()
            let legacyPillWidth: Double = 232 // builds before the 124 pt pill wrote no pillWidth
            let width = d.object(forKey: "pillWidth") as? Double ?? legacyPillWidth
            if let legacy = Layout.legacyOffset(x: d.double(forKey: "pillX"), y: d.double(forKey: "pillY"), width: width,
                                                screens: keyed.map { $0.screen.frame }) {
                d.set([keyed[legacy.index].uuid: legacy.offset], forKey: "pillOrigins")
            }
        }
        if d.object(forKey: "pillOrigins") != nil {
            for k in ["pillX", "pillY", "pillWidth"] { d.removeObject(forKey: k) }
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        migrateLegacyPillOrigin()
        syncPanels()
        NSApp.mainMenu = editMainMenu()

        // The Keychain read spawns `security`; keep it off the main thread.
        if AppDelegate.root == nil {
            let socketPath = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/state/cmux/last-socket-path")
            if let transport = SocketFeedTransport(pathFile: socketPath) { store.feedMonitor = FeedMonitor(transport: transport) }
            DispatchQueue.global(qos: .utility).async { [weak self] in
                let creds = Dictionary(uniqueKeysWithValues: Service.allCases.compactMap { s in Credentials.read(s).map { (s, $0) } })
                let bitbucket = creds[.bitbucket].map { BitbucketClient(account: $0.account, token: $0.token) }
                let jira = creds[.jira].map { JiraClient(account: $0.account, token: $0.token) }
                let accounts = creds.mapValues(\.account)
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    store.accounts = accounts
                    if let bitbucket {
                        bitbucketClient = bitbucket
                        store.prMonitor = PRMonitor(api: bitbucket)
                    }
                    if let jira {
                        jiraClient = jira
                        store.jiraMonitor = JiraMonitor(api: jira)
                    }
                }
            }
        }

        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            self?.syncPanels()
            // NSScreen.screens can be stale while the notification fires.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self?.syncPanels() }
        }
        let track: (NSEvent) -> Void = { [weak self] _ in self?.trackMouse() }
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: track) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: { track($0); return $0 }) { monitors.append(m) }

        store.muted = UserDefaults.standard.bool(forKey: "muted")
        store.$muted.dropFirst().sink { UserDefaults.standard.set($0, forKey: "muted") }.store(in: &cancellables)
        let d = UserDefaults.standard
        if let v = d.object(forKey: "bgOpacity") as? Double {
            store.bgOpacity = Layout.bgOpacities.min { abs($0 - v) < abs($1 - v) } ?? v
        }
        store.$bgOpacity.dropFirst().sink { UserDefaults.standard.set($0, forKey: "bgOpacity") }.store(in: &cancellables)
        if let p = d.string(forKey: "prPrompt") { store.prPrompt = p }
        store.$prPrompt.dropFirst().sink { UserDefaults.standard.set($0, forKey: "prPrompt") }.store(in: &cancellables)
        if let j = d.dictionary(forKey: "jiraProjects") as? [String: String] { store.jiraProjects = j }
        store.$jiraProjects.dropFirst().sink { UserDefaults.standard.set($0, forKey: "jiraProjects") }.store(in: &cancellables)

        if AppDelegate.root == nil { setUpHotKeys() }

        if AppDelegate.root == nil, SMAppService.mainApp.status != .enabled { try? SMAppService.mainApp.register() }
        refreshAndSyncVisibility()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            guard let self else { return }
            refreshAndSyncVisibility()
            trackMouse()
            tick += 1
            if tick.isMultiple(of: 2) { checkFocusedPane() }
        }
    }

    /// Loads the saved choices (absent key = default, stored "off" = none), registers them and keeps both in sync with the menu.
    func setUpHotKeys() {
        let d = UserDefaults.standard
        func load(_ key: String, default combo: HotKeyCombo) -> HotKeyCombo? {
            guard let v = d.object(forKey: key) else { return combo }
            return (v as? Data).flatMap { try? JSONDecoder().decode(HotKeyCombo.self, from: $0) }
        }
        func save(_ key: String, _ combo: HotKeyCombo?) {
            d.set(combo.flatMap { try? JSONEncoder().encode($0) } ?? "off", forKey: key)
        }
        store.hotkeyOpen = load("hotkeyOpen", default: HotKeyCombo.openChoices[0])
        store.hotkeyNext = load("hotkeyNext", default: HotKeyCombo.nextChoices[0])
        store.$hotkeyOpen.dropFirst().sink { save("hotkeyOpen", $0) }.store(in: &cancellables)
        store.$hotkeyNext.dropFirst().sink { save("hotkeyNext", $0) }.store(in: &cancellables)
        store.$hotkeyOpen.sink { [weak self] combo in
            guard let self else { return }
            let ok = hotKeys.register(combo, id: 1) { [weak self] in self?.toggleOpen() }
            if ok { store.hotkeyFailed.remove("open") } else { store.hotkeyFailed.insert("open") }
        }.store(in: &cancellables)
        store.$hotkeyNext.sink { [weak self] combo in
            guard let self else { return }
            let ok = hotKeys.register(combo, id: 2) { [weak self] in self?.jumpToNextWaiting() }
            if ok { store.hotkeyFailed.remove("next") } else { store.hotkeyFailed.insert("next") }
        }.store(in: &cancellables)
    }

    /// Closes any open card; otherwise opens and pins the one on the screen with the mouse.
    func toggleOpen() {
        if panels.values.contains(where: { $0.state.pinnedOpen || $0.state.hovering }) {
            for p in panels.values { p.state.pinnedOpen = false; p.state.pinnedEntered = false; p.state.hovering = false }
            return
        }
        let mouse = NSEvent.mouseLocation
        guard let p = panels.values.first(where: { $0.screen.contains(mouse) }) ?? panels.values.first else { return NSSound.beep() }
        store.peekUntil = nil
        p.state.pinnedOpen = true
        p.state.pinnedEntered = false
        p.state.hovering = true
        p.window.ignoresMouseEvents = false
    }

    /// Jumps to the session that has waited longest; repeated presses within 5 s move on. Sessions outside cmux are skipped.
    func jumpToNextWaiting() {
        let order = store.waitingOrder()
        let now = Date()
        guard let first = HotKeys.next(after: lastNext?.id, in: order, lastPress: lastNext?.at, now: now),
              var i = order.firstIndex(of: first) else { return NSSound.beep() }
        lastNext = (first, now)
        var left = order.count
        func attempt() {
            let id = order[i]
            resolveSurface(id) { [weak self] surface in
                guard let self else { return }
                guard let surface else {
                    left -= 1
                    guard left > 0 else { return NSSound.beep() }
                    i = (i + 1) % order.count
                    return attempt()
                }
                lastNext = (id, now)
                store.markSeen(id)
                if !CmuxJump.jump(surface: surface) { NSSound.beep() }
            }
        }
        attempt()
    }

    /// Plays this refresh's chime and opens the peek; when cmux is frontmost, sessions whose pane is focused are left out after an async cmux check.
    func refreshAndChime() {
        store.refresh()
        for p in panels.values {
            if let e = p.state.editing, !store.allows(e) { p.state.editing = nil }
        }
        let t = store.transitions, p = store.prChime
        if t.isEmpty || !cmuxFrontmost {
            return playChime(store.applyAlert(transitions: t, prChime: p, excluding: [], at: Date()))
        }
        silentIds(Array(t.keys)) { [weak self] silent in
            guard let self else { return }
            playChime(store.applyAlert(transitions: t, prChime: p, excluding: silent, at: Date()))
        }
    }

    private var cmuxFrontmost: Bool {
        NSWorkspace.shared.frontmostApplication?.bundleIdentifier == CmuxJump.bundleID
    }

    /// Calls `then` on main with the ids whose cmux surface is the focused one. Empty on any cmux failure.
    func silentIds(_ ids: [String], then: @escaping (Set<String>) -> Void) {
        var surfaces: [String: String] = [:]
        let group = DispatchGroup()
        for id in ids {
            group.enter()
            resolveSurface(id) { surface in
                if let surface { surfaces[id] = surface }
                group.leave()
            }
        }
        group.notify(queue: .main) {
            guard !surfaces.isEmpty else { return then([]) }
            DispatchQueue.global(qos: .utility).async {
                let focused = CmuxJump.focusedSurface()
                DispatchQueue.main.async {
                    guard let focused else { return then([]) }
                    then(Set(surfaces.filter { $0.value.caseInsensitiveCompare(focused) == .orderedSame }.keys))
                }
            }
        }
    }

    func playChime(_ chime: Chime?) {
        guard let chime, !store.muted else { return }
        NSSound(named: chime == .done ? "Glass" : "Funk")?.play()
    }

    func refreshAndSyncVisibility() {
        refreshAndChime()
        for p in panels.values {
            if store.hidden && p.window.isVisible {
                p.window.orderOut(nil)
                p.window.ignoresMouseEvents = true
            } else if !store.hidden && !p.window.isVisible {
                p.window.orderFrontRegardless()
            }
        }
    }

    /// Calls `then` on main with the session's cmux surface: the cached one, else the result of a `cmux sessions` lookup.
    /// Hits are cached in the store.
    func resolveSurface(_ id: String, then: @escaping (String?) -> Void) {
        if let surface = store.surfaces[id] { return then(surface) }
        let pid = store.pid(of: id)
        lookingUp.insert(id)
        DispatchQueue.global().async {
            let surface = CmuxJump.surface(sessionId: id)
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                lookingUp.remove(id)
                // The session's process changed while the lookup ran: the result may belong to the old one.
                if store.pid(of: id) == pid {
                    if let surface {
                        store.surfaces[id] = surface
                        store.surfaceMisses.remove(id)
                    } else {
                        store.surfaceMisses.insert(id)
                    }
                }
                then(surface)
            }
        }
    }

    /// Every 2 s: clears the marks of the session whose pane cmux shows. Starts nothing unless a mark exists and cmux is frontmost.
    func checkFocusedPane() {
        guard !store.unseen.isEmpty, cmuxFrontmost else { return clearFocus() }
        for id in store.unseen.keys where store.surfaces[id] == nil && !store.surfaceMisses.contains(id) && !lookingUp.contains(id) {
            resolveSurface(id) { _ in }
        }
        guard store.unseen.keys.contains(where: { store.surfaces[$0] != nil }) else { return clearFocus() }
        guard !checkingFocus else { return }
        checkingFocus = true
        let gen = focusGen
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let surface = CmuxJump.focusedSurface()
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                checkingFocus = false
                if focusGen == gen { store.noteFocused(surface) }
            }
        }
    }

    private func clearFocus() {
        focusGen += 1
        store.noteFocused(nil)
    }

    /// Entry of the connect dialog: one dialog at a time. The dialog re-shows itself through `showDialog`, keeping `connecting` set.
    func connect(_ service: Service) {
        if service == .jira && store.jiraHost == nil { return NSSound.beep() }
        guard !connecting else { return NSSound.beep() }
        connecting = true
        showDialog(service, notice: nil, email: nil)
    }

    /// Asks for an email and token, checks them against the service, stores them in the Keychain and starts the monitor.
    /// `notice` is the error of the previous attempt, shown above the instructions.
    private func showDialog(_ service: Service, notice: String?, email: String?) {
        let name = service.displayName
        let host = store.jiraHost
        let how: String
        switch service {
        case .bitbucket:
            how = "Tạo token bằng nút \"Create API token with scopes\": app Bitbucket, quyền read:user, read:repository, read:pullrequest."
        case .jira:
            how = "Tạo token bằng nút \"Create API token\" (loại classic) cho site \(host ?? "")."
        }
        let alert = NSAlert()
        alert.messageText = "Kết nối \(name)"
        alert.informativeText = (notice.map { $0 + "\n\n" } ?? "") + how
        let emailField = NSTextField(frame: NSRect(x: 0, y: 30, width: 320, height: 24))
        emailField.placeholderString = "Email Atlassian"
        emailField.stringValue = email ?? store.accounts[service] ?? ""
        let tokenField = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        tokenField.placeholderString = "API token"
        emailField.nextKeyView = tokenField
        let box = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 54))
        box.addSubview(emailField)
        box.addSubview(tokenField)
        alert.accessoryView = box
        alert.addButton(withTitle: "Lưu")
        alert.addButton(withTitle: "Huỷ")
        alert.addButton(withTitle: "Tạo token…")
        alert.window.initialFirstResponder = emailField.stringValue.isEmpty ? emailField : tokenField
        NSApp.activate(ignoringOtherApps: true)
        let response = alert.runModal()
        let account = emailField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let token = tokenField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        switch response {
        case .alertThirdButtonReturn:
            NSWorkspace.shared.open(Service.tokenPage)
            showDialog(service, notice: notice, email: account)
        case .alertFirstButtonReturn:
            guard !account.isEmpty, !token.isEmpty else {
                return showDialog(service, notice: "✗ Nhập cả email và token", email: account)
            }
            guard Credentials.isSafe(account), Credentials.isSafe(token) else {
                return showDialog(service, notice: "✗ Email hoặc token có ký tự không hợp lệ", email: account)
            }
            let existing: CredentialReloading? = service == .bitbucket ? bitbucketClient : jiraClient
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                let failure = Self.validate(service, account: account, token: token, host: host)
                    ?? (Credentials.write(service, account: account, token: token) ? nil : "✗ Không lưu được vào Keychain")
                if failure == nil { existing?.reload() }
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    if let failure { return showDialog(service, notice: failure, email: account) }
                    connecting = false
                    store.accounts[service] = account
                    guard existing == nil else { return }
                    switch service {
                    case .bitbucket:
                        let c = BitbucketClient(account: account, token: token)
                        bitbucketClient = c
                        store.prMonitor = PRMonitor(api: c)
                    case .jira:
                        let c = JiraClient(account: account, token: token)
                        jiraClient = c
                        store.jiraMonitor = JiraMonitor(api: c)
                    }
                }
            }
        default:
            connecting = false
        }
    }

    /// Calls the service with the typed credentials; the error line to show, or nil when they work. Background queue only.
    static func validate(_ service: Service, account: String, token: String, host: String?) -> String? {
        let name = service.displayName
        let site = service == .jira ? host ?? "" : "api.bitbucket.org"
        do {
            switch service {
            case .bitbucket: _ = try BitbucketClient(account: account, token: token).get("/2.0/user?fields=uuid")
            case .jira: _ = try JiraClient(account: account, token: token).myself(host: host ?? "")
            }
            return nil
        } catch {
            guard let code = ConnectionState.httpCode(of: error) else { return "✗ Không kết nối được tới \(site)" }
            return code == 401 ? "✗ \(name) trả về 401: email hoặc token sai" : "✗ Lỗi HTTP \(code)"
        }
    }

    func handle(_ a: WidgetAction) {
        if case .connect(let service) = a { return connect(service) }
        let id = a.sessionId
        if case .openLink(_, let url) = a {
            store.markSeen(id)
            NSWorkspace.shared.open(url)
            return
        }
        if case .nudgeRabbit(_, let ref) = a {
            guard let monitor = store.prMonitor else { return NSSound.beep() }
            monitor.nudge(ref) { ok in if !ok { NSSound.beep() } }
            return
        }
        if case .answer(_, let requestId, let answers, let other) = a {
            // Goes through cmux's socket, not the terminal: only a question still in the last `feed.list` of this session is answered.
            guard !sending.contains(id), let monitor = store.feedMonitor, store.stillValid(a),
                  let q = store.question(of: id) else { return NSSound.beep() }
            sending.insert(id)
            for p in panels.values where p.state.editing?.id == id { p.state.editing = nil }
            monitor.reply(q, answers: answers, other: other) { ok in
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    sending.remove(id)
                    if ok { return store.noteSent(session: id, requestId: requestId) }
                    for p in panels.values { p.state.failedQuestions.insert(requestId) }
                    NSSound.beep()
                }
            }
            return
        }
        if case .resume(let closed) = a {
            guard !sending.contains(id) else { return NSSound.beep() }
            sending.insert(id)
            DispatchQueue.global().async { [weak self] in
                let ok = closed.resume()
                DispatchQueue.main.async {
                    guard let self else { return }
                    self.sending.remove(id)
                    if ok { return self.store.closed.revive([id]) }
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(closed.shellCommand, forType: .string)
                    NSSound.beep()
                }
            }
            return
        }
        if case .jump = a {
            store.markSeen(id)
            resolveSurface(id) { surface in
                guard let surface, CmuxJump.jump(surface: surface) else { return NSSound.beep() }
            }
            return
        }
        guard !sending.contains(id) else { return NSSound.beep() }
        sending.insert(id)
        let denyReason: String? = {
            switch a {
            case .deny(_, _, let r), .reply(_, let r): return r
            default: return nil
            }
        }()
        // Close the editor now so a second Enter cannot queue a second send; the draft stays until the send succeeds.
        if denyReason != nil {
            for p in panels.values where p.state.editing?.id == id { p.state.editing = nil }
        }
        resolveSurface(id) { [weak self] surface in
            guard let self else { return }
            guard let surface else { sending.remove(id); return NSSound.beep() }
            // Validate right before the first keystroke: the lookup above may have taken a while.
            refreshAndChime()
            guard store.surfaces[id] == surface, store.stillValid(a) else {
                sending.remove(id)
                return NSSound.beep()
            }
            CmuxJump.send(surface: surface, keys: CmuxJump.keys(for: a)) { [weak self] ok in
                guard let self else { return }
                sending.remove(id)
                guard ok else { return NSSound.beep() }
                store.markSeen(id)
                if let denyReason, store.drafts[id] == denyReason { store.drafts[id] = nil }
            }
        }
    }

    @discardableResult
    func place(_ p: Panel, pillOrigin raw: CGPoint, save: Bool) -> [Double] {
        let s = p.screen
        let o = Layout.clamp(pillOrigin: raw, to: s)
        let offset = Layout.offset(of: o, in: s)
        p.state.anchor = Anchor.near(Layout.pillCentre(o), in: s)
        p.window.setFrame(Layout.canvasFrame(pillOrigin: o, anchor: p.state.anchor), display: true)
        if save {
            var all = UserDefaults.standard.dictionary(forKey: "pillOrigins") as? [String: [Double]] ?? [:]
            all[p.uuid] = offset
            UserDefaults.standard.set(all, forKey: "pillOrigins")
        }
        return offset
    }

    /// After a drop `place` re-anchors the pill at `Layout.settledPillOrigin` and the card re-lays out around it.
    func settleAfterDrop(_ p: Panel, start: CGPoint, raw: CGPoint, clamped: CGPoint) {
        place(p, pillOrigin: Layout.settledPillOrigin(start: start, raw: raw, clamped: clamped, size: p.window.frame.size, anchor: p.state.anchor, screen: p.screen), save: true)
        trackMouse()
    }

    func trackMouse() {
        if store.hidden { return }
        let mouse = NSEvent.mouseLocation
        for p in panels.values {
            // The pill lies inside the fixed canvas frame, so most mouse moves skip the layout computation.
            let inside = p.window.frame.contains(mouse) && Layout.pill(in: p.window.frame, size: Layout.size(store, p.state), anchor: p.state.anchor).contains(mouse)
            var want = inside
            if p.state.pinnedOpen {
                if inside { p.state.pinnedEntered = true }
                if !inside && p.state.pinnedEntered { p.state.pinnedOpen = false; p.state.pinnedEntered = false } else { want = true }
            }
            if p.state.editing == nil && want != p.state.hovering {
                p.state.hovering = want
                if want { store.peekUntil = nil }
            }
            if !inside && p.state.hoveredRow != nil { p.state.hoveredRow = nil }
            // While editing, clicks outside the card must reach the app beneath (and make the panel resign key).
            if p.window.ignoresMouseEvents == want { p.window.ignoresMouseEvents = !want }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()

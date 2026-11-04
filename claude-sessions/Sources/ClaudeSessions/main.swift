import AppKit
import ClaudeSessionsKit
import Combine
import ServiceManagement

final class AppDelegate: NSObject, NSApplicationDelegate {
    let store = Store(startHidden: true)
    let state = PanelState()
    var panel: WidgetPanel!
    var timer: Timer?
    var monitors: [Any] = []
    var sending: Set<String> = []
    var cancellables: Set<AnyCancellable> = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        panel = WidgetPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.acceptsMouseMovedEvents = true
        panel.state = state
        let host = DragHostingView(rootView: WidgetView(store: store, panel: state, onAction: { [weak self] in self?.handle($0) }))
        host.onDragEnd = { [weak self] in self?.settleAfterDrop() }
        panel.contentView = host

        state.$editing.sink { [weak self] e in
            guard let panel = self?.panel else { return }
            // @Published fires before the value is stored, so defer until canBecomeKey sees it.
            DispatchQueue.main.async {
                guard let store = self?.store else { return }
                if e != nil {
                    if !store.hidden { panel.makeKey() }
                } else if panel.isKeyWindow && !store.hidden {
                    // resignKey() is only a callback; reordering the panel is what hands key status back.
                    panel.orderOut(nil)
                    panel.orderFrontRegardless()
                }
            }
        }.store(in: &cancellables)
        NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: panel, queue: .main) { [weak self] _ in
            self?.state.editing = nil
        }

        let d = UserDefaults.standard
        if d.object(forKey: "pillX") != nil {
            var x = d.double(forKey: "pillX")
            let y = d.double(forKey: "pillY")
            let legacyPillWidth: Double = 232 // builds before the 124 pt pill wrote no pillWidth
            let w = d.object(forKey: "pillWidth") as? Double ?? legacyPillWidth
            if w != Layout.collapsed.width {
                let centre = CGPoint(x: x + w / 2, y: y + Layout.collapsed.height / 2)
                if let f = screenFrame(containing: centre) {
                    x = Layout.migrate(pillX: x, from: w, to: Layout.collapsed.width, anchor: Anchor.near(centre, in: f))
                }
            }
            place(pillOrigin: CGPoint(x: x, y: y))
        } else if let s = NSScreen.main?.visibleFrame {
            place(pillOrigin: CGPoint(x: s.maxX - Layout.collapsed.width - 16, y: s.maxY - Layout.collapsed.height - 10))
        }

        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            self?.resettle()
        }
        let track: (NSEvent) -> Void = { [weak self] _ in self?.trackMouse() }
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: track) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: { track($0); return $0 }) { monitors.append(m) }

        store.muted = UserDefaults.standard.bool(forKey: "muted")
        store.$muted.dropFirst().sink { UserDefaults.standard.set($0, forKey: "muted") }.store(in: &cancellables)
        if let v = d.object(forKey: "bgOpacity") as? Double {
            store.bgOpacity = Layout.bgOpacities.min { abs($0 - v) < abs($1 - v) } ?? v
        }
        store.$bgOpacity.dropFirst().sink { UserDefaults.standard.set($0, forKey: "bgOpacity") }.store(in: &cancellables)

        if SMAppService.mainApp.status != .enabled { try? SMAppService.mainApp.register() }
        refreshAndSyncVisibility()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.refreshAndSyncVisibility()
            self?.trackMouse()
        }
    }

    /// `refresh()` clears the chime, so every refresh plays it right away.
    func refreshAndChime() {
        store.refresh()
        if let e = state.editing, !store.allows(e) { state.editing = nil }
        playChime()
    }

    func playChime() {
        guard let chime = store.chime, !store.muted else { return }
        NSSound(named: chime == .done ? "Glass" : "Funk")?.play()
    }

    func refreshAndSyncVisibility() {
        refreshAndChime()
        if store.hidden && panel.isVisible {
            panel.orderOut(nil)
            panel.ignoresMouseEvents = true
        } else if !store.hidden && !panel.isVisible {
            panel.orderFrontRegardless()
        }
    }

    /// Calls `then` on main with the session's cmux surface: the cached one, else the result of a `cmux sessions` lookup.
    /// Hits are cached in the store.
    func resolveSurface(_ id: String, then: @escaping (String?) -> Void) {
        if let surface = store.surfaces[id] { return then(surface) }
        let pid = store.pid(of: id)
        DispatchQueue.global().async {
            let surface = CmuxJump.surface(sessionId: id)
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                // The session's process changed while the lookup ran: the result may belong to the old one.
                if store.pid(of: id) == pid {
                    if let surface { store.surfaces[id] = surface }
                }
                then(surface)
            }
        }
    }

    func handle(_ a: WidgetAction) {
        let id = a.sessionId
        if case .openLink(_, let url) = a {
            NSWorkspace.shared.open(url)
            return
        }
        if case .jump = a {
            resolveSurface(id) { surface in
                guard let surface, CmuxJump.jump(surface: surface) else { return NSSound.beep() }
            }
            return
        }
        guard !sending.contains(id) else { return NSSound.beep() }
        sending.insert(id)
        let denyReason: String? = { if case .deny(_, _, let r) = a { return r }; return nil }()
        // Close the editor now so a second Enter cannot queue a second send; the draft stays until the send succeeds.
        if denyReason != nil { state.editing = nil }
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
                if let denyReason, store.drafts[id] == denyReason { store.drafts[id] = nil }
            }
        }
    }

    func place(pillOrigin raw: CGPoint) {
        let center = CGPoint(x: raw.x + Layout.collapsed.width / 2, y: raw.y + Layout.collapsed.height / 2)
        guard let s = screenFrame(containing: center) else { return }
        let o = Layout.clamp(pillOrigin: raw, to: s)
        state.anchor = Anchor.near(CGPoint(x: o.x + Layout.collapsed.width / 2, y: o.y + Layout.collapsed.height / 2), in: s)
        panel.setFrame(Layout.canvasFrame(pillOrigin: o, anchor: state.anchor), display: true)
        UserDefaults.standard.set(Double(o.x), forKey: "pillX")
        UserDefaults.standard.set(Double(o.y), forKey: "pillY")
        UserDefaults.standard.set(Double(Layout.collapsed.width), forKey: "pillWidth")
    }

    private func screenFrame(containing p: CGPoint) -> NSRect? {
        NSScreen.screens.first { $0.frame.contains(p) }?.frame ?? NSScreen.main?.frame
    }

    /// After a drop the card stays where it was dropped and the pill moves to its corner.
    func settleAfterDrop() {
        let card = Layout.pill(in: panel.frame, size: Layout.size(store, state), anchor: state.anchor)
        guard let s = screenFrame(containing: CGPoint(x: card.midX, y: card.midY)) else { return }
        place(pillOrigin: Layout.settledPillOrigin(card: card, screen: s))
        trackMouse()
    }

    /// After a screen change the pill stays put and is only re-clamped to the screens.
    func resettle() {
        place(pillOrigin: Layout.pill(in: panel.frame, size: Layout.collapsed, anchor: state.anchor).origin)
        trackMouse()
    }

    func trackMouse() {
        if store.hidden { return }
        let inside = Layout.pill(in: panel.frame, size: Layout.size(store, state), anchor: state.anchor).contains(NSEvent.mouseLocation)
        if state.editing == nil && inside != state.hovering {
            state.hovering = inside
            if inside { store.peekUntil = nil }
        }
        // While editing, clicks outside the card must reach the app beneath (and make the panel resign key).
        panel.ignoresMouseEvents = !inside
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()

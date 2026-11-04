import AppKit
import ClaudeSessionsKit
import Combine
import ServiceManagement

final class Panel {
    let uuid: String
    let window: WidgetPanel
    let state: PanelState
    var cancellables: Set<AnyCancellable> = []

    init(uuid: String, window: WidgetPanel, state: PanelState) {
        self.uuid = uuid
        self.window = window
        self.state = state
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    let store = Store(startHidden: true)
    var panels: [String: Panel] = [:]
    var timer: Timer?
    var monitors: [Any] = []
    var sending: Set<String> = []
    var cancellables: Set<AnyCancellable> = []

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

    func frame(of uuid: String) -> NSRect? {
        keyedScreens().first { $0.uuid == uuid }?.screen.frame
    }

    func makePanel(uuid: String) -> Panel {
        let window = WidgetPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
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
        host.onDragEnd = { [weak self, weak p] in
            guard let self, let p else { return }
            settleAfterDrop(p)
        }
        host.clampOrigin = { [weak self, weak p] origin in
            guard let self, let p, let screen = frame(of: p.uuid) else { return origin }
            return Layout.clampDrag(windowOrigin: origin, windowSize: p.window.frame.size, visible: Layout.size(store, p.state), anchor: p.state.anchor, screen: screen)
        }
        window.contentView = host

        state.$editing.sink { [weak self, weak window] e in
            guard let window else { return }
            // @Published fires before the value is stored, so defer until canBecomeKey sees it.
            DispatchQueue.main.async {
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
        NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: window, queue: .main) { [weak state] _ in
            state?.editing = nil
        }
        if !store.hidden { window.orderFrontRegardless() }
        return p
    }

    func syncPanels() {
        let keyed = keyedScreens()
        for (uuid, p) in panels where !keyed.contains(where: { $0.uuid == uuid }) {
            p.window.orderOut(nil)
            p.window.close()
            panels[uuid] = nil
        }

        let d = UserDefaults.standard
        if d.object(forKey: "pillX") != nil {
            if d.object(forKey: "pillOrigins") == nil, let first = keyed.first {
                var x = d.double(forKey: "pillX")
                let y = d.double(forKey: "pillY")
                let legacyPillWidth: Double = 232 // builds before the 124 pt pill wrote no pillWidth
                let w = d.object(forKey: "pillWidth") as? Double ?? legacyPillWidth
                let centre = CGPoint(x: x + w / 2, y: y + Layout.collapsed.height / 2)
                let target = keyed.first { $0.screen.frame.contains(centre) } ?? first
                let f = target.screen.frame
                if w != Layout.collapsed.width {
                    x = Layout.migrate(pillX: x, from: w, to: Layout.collapsed.width, anchor: Anchor.near(centre, in: f))
                }
                let o = Layout.clamp(pillOrigin: CGPoint(x: x, y: y), to: f)
                d.set([target.uuid: Layout.offset(of: o, in: f)], forKey: "pillOrigins")
            }
            if d.object(forKey: "pillOrigins") != nil {
                for k in ["pillX", "pillY", "pillWidth"] { d.removeObject(forKey: k) }
            }
        }

        func saved(_ uuid: String) -> [Double]? {
            (d.dictionary(forKey: "pillOrigins") as? [String: [Double]])?[uuid]
        }
        for (uuid, screen) in keyed {
            let p = panels[uuid] ?? makePanel(uuid: uuid)
            panels[uuid] = p
            let s = screen.frame
            if let o = saved(uuid).flatMap({ Layout.origin(offset: $0, in: s) }) {
                place(p, pillOrigin: o, save: false)
            } else if let ref = keyed.first(where: { saved($0.uuid) != nil }),
                      let refOrigin = saved(ref.uuid).flatMap({ Layout.origin(offset: $0, in: ref.screen.frame) }) {
                let rf = ref.screen.frame
                let centre = CGPoint(x: refOrigin.x + Layout.collapsed.width / 2, y: refOrigin.y + Layout.collapsed.height / 2)
                place(p, pillOrigin: Layout.relativeOrigin(refOrigin, from: rf, to: s, anchor: Anchor.near(centre, in: rf)), save: true)
            } else {
                let v = screen.visibleFrame
                place(p, pillOrigin: CGPoint(x: v.maxX - Layout.collapsed.width - 16, y: v.maxY - Layout.collapsed.height - 10), save: true)
            }
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        syncPanels()

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
        for p in panels.values {
            if let e = p.state.editing, !store.allows(e) { p.state.editing = nil }
        }
        playChime()
    }

    func playChime() {
        guard let chime = store.chime, !store.muted else { return }
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
                if let denyReason, store.drafts[id] == denyReason { store.drafts[id] = nil }
            }
        }
    }

    func place(_ p: Panel, pillOrigin raw: CGPoint, save: Bool) {
        guard let s = frame(of: p.uuid) else { return }
        let o = Layout.clamp(pillOrigin: raw, to: s)
        p.state.anchor = Anchor.near(CGPoint(x: o.x + Layout.collapsed.width / 2, y: o.y + Layout.collapsed.height / 2), in: s)
        p.window.setFrame(Layout.canvasFrame(pillOrigin: o, anchor: p.state.anchor), display: true)
        if save {
            var all = UserDefaults.standard.dictionary(forKey: "pillOrigins") as? [String: [Double]] ?? [:]
            all[p.uuid] = Layout.offset(of: o, in: s)
            UserDefaults.standard.set(all, forKey: "pillOrigins")
        }
    }

    /// After a drop the card stays where it was dropped and the pill moves to its corner.
    func settleAfterDrop(_ p: Panel) {
        let card = Layout.pill(in: p.window.frame, size: Layout.size(store, p.state), anchor: p.state.anchor)
        guard let s = frame(of: p.uuid) else { return }
        place(p, pillOrigin: Layout.settledPillOrigin(card: card, screen: s), save: true)
        trackMouse()
    }

    func trackMouse() {
        if store.hidden { return }
        for p in panels.values {
            let inside = Layout.pill(in: p.window.frame, size: Layout.size(store, p.state), anchor: p.state.anchor).contains(NSEvent.mouseLocation)
            if p.state.editing == nil && inside != p.state.hovering {
                p.state.hovering = inside
                if inside { store.peekUntil = nil }
            }
            // While editing, clicks outside the card must reach the app beneath (and make the panel resign key).
            p.window.ignoresMouseEvents = !inside
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()

import AppKit
import ClaudeSessionsKit
import Combine
import ServiceManagement

final class AppDelegate: NSObject, NSApplicationDelegate {
    let store = Store(startHidden: true)
    var panel: WidgetPanel!
    var timer: Timer?
    var monitors: [Any] = []
    var lookingUp: Set<String> = []
    var tick = 0
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
        panel.store = store
        let host = DragHostingView(rootView: WidgetView(store: store, onAction: { [weak self] in self?.handle($0) }))
        host.onDragEnd = { [weak self] in self?.settle() }
        panel.contentView = host

        store.$editing.sink { [weak self] e in
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
            self?.store.editing = nil
        }

        let d = UserDefaults.standard
        if d.object(forKey: "pillX") != nil {
            place(pillOrigin: CGPoint(x: d.double(forKey: "pillX"), y: d.double(forKey: "pillY")))
        } else if let s = NSScreen.main?.visibleFrame {
            place(pillOrigin: CGPoint(x: s.maxX - Layout.collapsed.width - 16, y: s.maxY - Layout.collapsed.height - 10))
        }

        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            self?.settle()
        }
        let track: (NSEvent) -> Void = { [weak self] _ in self?.trackMouse() }
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: track) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: { track($0); return $0 }) { monitors.append(m) }

        if SMAppService.mainApp.status != .enabled { try? SMAppService.mainApp.register() }
        refreshAndSyncVisibility()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.refreshAndSyncVisibility()
            self?.trackMouse()
            self?.tick += 1
            if self?.tick.isMultiple(of: 2) == true { self?.checkFocusedPane() }
        }
    }

    func refreshAndSyncVisibility() {
        store.refresh()
        if store.hidden && panel.isVisible {
            panel.orderOut(nil)
            panel.ignoresMouseEvents = true
        } else if !store.hidden && !panel.isVisible {
            panel.orderFrontRegardless()
        }
    }

    /// Calls `then` on main with the session's cmux surface: the cached one, else the result of a `cmux sessions` lookup.
    /// Hits are cached in the store; misses are recorded there too, but only the background check consults them.
    func resolveSurface(_ id: String, then: @escaping (String?) -> Void) {
        if let surface = store.surfaces[id] { return then(surface) }
        lookingUp.insert(id)
        DispatchQueue.global().async {
            let surface = CmuxJump.surface(sessionId: id)
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                lookingUp.remove(id)
                if let surface {
                    store.surfaces[id] = surface
                    store.surfaceMisses.remove(id)
                } else {
                    store.surfaceMisses.insert(id)
                }
                then(surface)
            }
        }
    }

    func handle(_ a: WidgetAction) {
        let id = a.sessionId
        resolveSurface(id) { [weak self] surface in
            guard let self else { return }
            guard let surface else { return NSSound.beep() }
            if case .jump = a {
                if CmuxJump.jump(surface: surface) { store.markSeen(id) } else { NSSound.beep() }
                return
            }
            // Validate right before the first keystroke: the lookup above may have taken a while.
            store.refresh()
            let fromEditor = store.editing?.id == id
            guard store.stillValid(a) else {
                NSSound.beep()
                if fromEditor { store.editing = nil }
                return
            }
            if fromEditor { store.editing = nil }
            CmuxJump.send(surface: surface, keys: CmuxJump.keys(for: a)) { [weak self] ok in
                guard let self else { return }
                guard ok else { return NSSound.beep() }
                if fromEditor { store.drafts[id] = nil }
                store.markSeen(id)
            }
        }
    }

    /// Marks an unseen session as seen when its cmux pane is the focused one.
    func checkFocusedPane() {
        guard !store.unseen.isEmpty,
              NSWorkspace.shared.frontmostApplication?.bundleIdentifier == CmuxJump.bundleID else { return }
        for id in store.unseen where store.surfaces[id] == nil && !store.surfaceMisses.contains(id) && !lookingUp.contains(id) {
            resolveSurface(id) { _ in }
        }
        guard store.unseen.contains(where: { store.surfaces[$0] != nil }), let focused = CmuxJump.focusedSurface() else { return }
        for id in store.unseen where store.surfaces[id]?.caseInsensitiveCompare(focused) == .orderedSame { store.markSeen(id) }
    }

    func place(pillOrigin raw: CGPoint) {
        let center = CGPoint(x: raw.x + Layout.collapsed.width / 2, y: raw.y + Layout.collapsed.height / 2)
        let screen = NSScreen.screens.first { $0.frame.contains(center) } ?? NSScreen.main
        guard let s = screen?.frame else { return }
        let o = Layout.clamp(pillOrigin: raw, to: s)
        store.anchor = Anchor.near(CGPoint(x: o.x + Layout.collapsed.width / 2, y: o.y + Layout.collapsed.height / 2), in: s)
        panel.setFrame(Layout.canvasFrame(pillOrigin: o, anchor: store.anchor), display: true)
        UserDefaults.standard.set(Double(o.x), forKey: "pillX")
        UserDefaults.standard.set(Double(o.y), forKey: "pillY")
    }

    func settle() {
        let size = Layout.size(store)
        let pill = Layout.pill(in: panel.frame, size: size, anchor: store.anchor)
        let x: CGFloat
        switch store.anchor.h {
        case .leading: x = pill.minX
        case .center: x = pill.midX - Layout.collapsed.width / 2
        case .trailing: x = pill.maxX - Layout.collapsed.width
        }
        let y = store.anchor.up ? pill.minY : pill.maxY - Layout.collapsed.height
        place(pillOrigin: CGPoint(x: x, y: y))
        trackMouse()
    }

    func trackMouse() {
        if store.hidden { return }
        let inside = Layout.pill(in: panel.frame, size: Layout.size(store), anchor: store.anchor).contains(NSEvent.mouseLocation)
        if store.editing == nil && inside != store.hovering {
            store.hovering = inside
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

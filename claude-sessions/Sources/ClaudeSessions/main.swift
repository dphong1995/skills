import AppKit
import ClaudeSessionsKit
import Combine

final class AppDelegate: NSObject, NSApplicationDelegate {
    let store = Store()
    var panel: WidgetPanel!
    var timer: Timer?
    var monitors: [Any] = []
    var lookingUp: Set<String> = []
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
            DispatchQueue.main.async { if e != nil { panel.makeKey() } else { panel.resignKey() } }
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

        store.refresh()
        panel.orderFrontRegardless()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.store.refresh()
            self?.trackMouse()
            self?.checkFocusedPane()
        }
    }

    func handle(_ a: WidgetAction) {
        let id = a.sessionId
        if case .jump = a {
            CmuxJump.jump(sessionId: id)
            store.markSeen(id)
            return
        }
        store.refresh()
        guard store.stillValid(a) else {
            NSSound.beep()
            store.editing = nil
            return
        }
        CmuxJump.send(sessionId: id, keys: CmuxJump.keys(for: a), knownSurface: store.surfaces[id])
        store.markSeen(id)
        switch a {
        case .reply, .deny:
            store.editing = nil
            store.drafts[id] = nil
        default: break
        }
    }

    /// Marks an unseen session as seen when its cmux pane is the focused one.
    func checkFocusedPane() {
        guard !store.unseen.isEmpty,
              NSWorkspace.shared.frontmostApplication?.bundleIdentifier == "com.cmuxterm.app" else { return }
        for id in store.unseen where store.surfaces[id] == nil && !lookingUp.contains(id) {
            lookingUp.insert(id)
            DispatchQueue.global().async { [store] in
                let surface = CmuxJump.surface(sessionId: id)
                DispatchQueue.main.async { [weak self] in
                    self?.lookingUp.remove(id)
                    if let surface { store.surfaces[id] = surface }
                }
            }
        }
        guard let focused = CmuxJump.focusedSurface() else { return }
        for id in store.unseen where store.surfaces[id] == focused { store.markSeen(id) }
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
        let zone = Layout.pill(in: panel.frame, size: Layout.size(store), anchor: store.anchor)
        if store.editing != nil {
            panel.ignoresMouseEvents = false
            return
        }
        let inside = zone.contains(NSEvent.mouseLocation)
        if inside != store.hovering {
            store.hovering = inside
            if inside { store.peekUntil = nil }
        }
        panel.ignoresMouseEvents = !inside
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()

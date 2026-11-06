import SwiftUI

/// State that belongs to one pill (one display); everything shared lives in `Store`.
public final class PanelState: ObservableObject {
    @Published public var hovering = false
    /// Opened by the hotkey: `trackMouse` keeps `hovering` true until the hotkey is pressed again or the mouse has entered and then left the card.
    public var pinnedOpen = false
    public var pinnedEntered = false
    @Published public var editing: Store.Editing?
    @Published public var anchor = Anchor()
    @Published public var measuredHeight: CGFloat?
    /// Height of the session rows alone, so the list can scroll when the panel is full.
    @Published public var listHeight: CGFloat?

    /// Session ids whose `☰ N` list is expanded; cleared when the widget collapses.
    @Published public var openMenus: Set<String> = []

    /// Session id the mouse is over in the expanded card; drives the hover-dwell that clears a fresh mark.
    @Published public var hoveredRow: String?

    public init() {}

    public func noteRowHover(_ id: String, inside: Bool) {
        if inside {
            if hoveredRow != id { hoveredRow = id }
        } else if hoveredRow == id {
            hoveredRow = nil
        }
    }

    /// Clears `hoveredRow` when its position in the id order changed or it vanished.
    public func noteOrderChange(from old: [String], to new: [String]) {
        guard let id = hoveredRow, old.firstIndex(of: id) != new.firstIndex(of: id) else { return }
        hoveredRow = nil
    }

    /// The hovered row when it carries a fresh mark.
    public func dwellTarget(in store: Store) -> String? {
        hoveredRow.flatMap { store.unseen[$0] != nil ? $0 : nil }
    }

    public func expanded(_ store: Store) -> Bool { hovering || editing != nil || store.peeking }
}

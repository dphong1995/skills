import SwiftUI

/// State that belongs to one pill (one display); everything shared lives in `Store`.
public final class PanelState: ObservableObject {
    @Published public var hovering = false
    @Published public var editing: Store.Editing?
    @Published public var anchor = Anchor()
    @Published public var measuredHeight: CGFloat?
    /// Height of the session rows alone, so the list can scroll when the panel is full.
    @Published public var listHeight: CGFloat?

    /// Session ids whose `☰ N` list is expanded; cleared when the widget collapses.
    @Published public var openMenus: Set<String> = []

    public init() {}

    public func expanded(_ store: Store) -> Bool { hovering || editing != nil || store.peeking }
}

import AppKit
import SwiftUI

extension WidgetView {
    func header(expanded: Bool) -> some View {
        Group {
            if expanded {
                groups(expanded: true, dot: 3, gap: 7)
                    .frame(maxWidth: .infinity, alignment: .trailing)
            } else {
                // Two-digit counts fall back to the tight spacing.
                ViewThatFits(in: .horizontal) {
                    groups(expanded: false, dot: 6, gap: 10)
                    groups(expanded: false, dot: 3, gap: 6)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .shadow(color: expanded ? .clear : Color.black.opacity(0.6), radius: 1.5)
        .padding(.horizontal, expanded ? 18 : 12)
    }

    func groups(expanded: Bool, dot: CGFloat, gap: CGFloat) -> some View {
        let counts = Dictionary(grouping: store.sessions, by: \.colorStatus).mapValues(\.count)
        let newCount = store.newCount
        return HStack(spacing: gap) {
            if let q = store.quota {
                if expanded {
                    quotaGroup(q)
                } else if let p = q.pillPercent {
                    Text("\(p)%").font(Theme.Fonts.count.monospacedDigit()).foregroundStyle(quotaColor(p)).fixedSize()
                }
            }
            if newCount > 0 {
                HStack(spacing: dot) {
                    Image(systemName: Theme.Symbols.fresh).font(Theme.Fonts.smallStrong).fixedSize()
                    Text("\(newCount)").font(Theme.Fonts.count.monospacedDigit()).fixedSize()
                }
                .foregroundStyle(Theme.Colors.fresh)
            }
            if store.sessions.isEmpty {
                HStack(spacing: 6) {
                    if !expanded { Circle().fill(.gray).frame(width: 6, height: 6) }
                    Text("–").font(Theme.Fonts.label).foregroundStyle(Theme.Colors.tertiaryText)
                }
            }
            ForEach(["waiting", "idle", "busy"], id: \.self) { st in
                if let n = counts[st] {
                    HStack(spacing: dot) {
                        Circle().fill(color(st)).frame(width: 6, height: 6)
                        Text("\(n)").font(Theme.Fonts.count.monospacedDigit()).foregroundStyle(Theme.Colors.primaryText).fixedSize()
                    }
                }
            }
        }
    }

    func quotaColor(_ percent: Int) -> Color {
        switch QuotaLevel.level(percent) {
        case .normal: Theme.Colors.primaryText
        case .warn: .yellow
        case .high: .red
        }
    }

    static let clock: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm"
        return f
    }()

    func quotaGroup(_ q: Quota) -> some View {
        let stale = q.isStale(now: store.now)
        let t = Self.clock
        return HStack(spacing: 4) {
            Text("5h \(q.fiveHour)%").foregroundStyle(quotaColor(q.fiveHour))
            Text("· \(t.string(from: q.fiveHourReset))")
            if let f = q.forecast { Text("→ \(t.string(from: f))") }
            Text("· tuần \(q.weekly)%").foregroundStyle(quotaColor(q.weekly))
            if stale { Text("cũ") }
        }
        .font(Theme.Fonts.smallStrong.monospacedDigit())
        .foregroundStyle(Theme.Colors.secondaryText)
        .lineLimit(1).fixedSize()
        .opacity(stale ? Theme.Opacity.stale : 1)
        .padding(.trailing, 6)
    }
}

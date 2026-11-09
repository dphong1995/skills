import AppKit
import SwiftUI

extension WidgetView {
    func header(expanded: Bool) -> some View {
        Group {
            if expanded {
                HStack(spacing: Theme.Space.gap) {
                    Spacer(minLength: Theme.Space.gap)
                    HStack(spacing: Theme.Space.quotaToCounts) {
                        if let q = store.quota { quotaBars(q) }
                        groups(expanded: true, dot: 3, gap: 7, showQuota: false)
                    }
                }
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

    func groups(expanded: Bool, dot: CGFloat, gap: CGFloat, showQuota: Bool = true) -> some View {
        let counts = Dictionary(grouping: store.sessions, by: \.colorStatus).mapValues(\.count)
        let newCount = store.newCount
        return HStack(spacing: gap) {
            if showQuota, !expanded, let p = store.quota?.pillPercent {
                HStack(spacing: 3) {
                    Text("5h").font(Theme.Fonts.meta).foregroundStyle(Theme.Colors.tertiaryText)
                    Text("\(p)%").font(Theme.Fonts.count.monospacedDigit()).foregroundStyle(quotaColor(p))
                }
                .fixedSize()
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
                    if st == "waiting" {
                        Text("\(n)")
                            .font(Theme.Fonts.count.monospacedDigit())
                            .foregroundStyle(Color.white)
                            .padding(.horizontal, Theme.Space.badgeH).padding(.vertical, Theme.Space.badgeV)
                            .background(Capsule().fill(Theme.Colors.status(st)))
                            .fixedSize()
                    } else {
                        HStack(spacing: dot) {
                            Circle().fill(Theme.Colors.status(st)).frame(width: 6, height: 6)
                            Text("\(n)").font(Theme.Fonts.count.monospacedDigit()).foregroundStyle(Theme.Colors.secondaryText).fixedSize()
                        }
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

    func quotaBar(_ label: String, _ percent: Int) -> some View {
        HStack(spacing: 4) {
            Text(label).font(Theme.Fonts.meta).foregroundStyle(Theme.Colors.tertiaryText)
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.Colors.field)
                Capsule().fill(quotaColor(percent))
                    .frame(width: Theme.Space.quotaBarWidth * CGFloat(min(max(percent, 0), 100)) / 100)
            }
            .frame(width: Theme.Space.quotaBarWidth, height: Theme.Space.quotaBarHeight)
            Text("\(percent)%").font(Theme.Fonts.metaStrong.monospacedDigit()).foregroundStyle(quotaColor(percent))
        }
    }

    func quotaBars(_ q: Quota) -> some View {
        let stale = q.isStale(now: store.now)
        let t = Self.clock
        return HStack(spacing: Theme.Space.gap + 2) {
            quotaBar("5h", q.fiveHour)
            if let f = q.forecast {
                HStack(spacing: 3) {
                    Image(systemName: Theme.Symbols.warning)
                    Text("hết ~\(t.string(from: f))")
                }
                .font(Theme.Fonts.metaStrong.monospacedDigit())
                .foregroundStyle(Theme.Colors.status("waiting"))
            } else {
                Text("reset \(t.string(from: q.fiveHourReset))")
                    .font(Theme.Fonts.meta.monospacedDigit()).foregroundStyle(Theme.Colors.tertiaryText)
            }
            quotaBar("Tuần", q.weekly)
            if stale { Text("cũ").font(Theme.Fonts.meta).foregroundStyle(Theme.Colors.tertiaryText) }
        }
        .lineLimit(1).fixedSize()
        .opacity(stale ? Theme.Opacity.stale : 1)
    }
}

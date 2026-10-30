// Compacts the main conversation on its own: after a turn that ends at or above `threshold`,
// mid-turn at `hardLimit` (the turn is ended, compacted, then continued by a prompt), and
// `idleMinutes` after a turn that ended at or above `idleThreshold` if no turn started since.
// Subagent events and headless sessions pass through untouched.
import type { On, PluginOptions, Register } from 'claude-code'

const INSTRUCTIONS =
  "Keep what is needed to continue without re-reading the transcript: the user's latest request and whether it is answered; the skill or workflow in progress (name, step, round) and the paths of its plan, spec and phase files; decisions made and why; files changed and their state; commands and tests still to run with their last results; background agents and commands still running and what each is for; questions waiting for the user."

const CONTINUE =
  'The context was compacted in the middle of your last turn. Continue the step you were on. Check what is already done before redoing anything; background agents that are still running will report here.'

let stopPending = false
let hardStopOff = false
let idleTimer: { cancel: () => void } | undefined

type Engine = Parameters<Parameters<On>[2]>[0]

const compact = ($: Engine) =>
  $.session.compact({ instructions: INSTRUCTIONS }).then((r) => !('skip' in r))

export const register: Register = (on, options: PluginOptions) => {
  const threshold = Number(options.threshold)
  const hardLimit = Number(options.hardLimit)
  const idleThreshold = Number(options.idleThreshold)
  const idleMs = Number(options.idleMinutes) * 60_000

  on('turn.start', async ($, e, next) => {
    if (!e.agentId) idleTimer?.cancel()
    return next(e)
  })

  on('turn.step', async function* ($, e, next) {
    if (!e.agentId && e.index > 0 && (await $.session.surfaces()).length > 0) {
      const tokens = (await $.session.usage()).context.tokens
      if (tokens !== undefined && tokens >= hardLimit && !hardStopOff && !stopPending) {
        stopPending = true
        const text = `Context ${Math.round(tokens / 1000)}k ≥ ${Math.round(hardLimit / 1000)}k: dừng turn để compact, xong sẽ tự làm tiếp.`
        yield { kind: 'text', index: 0, text }
        yield { kind: 'stop', stopReason: 'end_turn', usage: null }
        return { turnId: e.turnId, index: e.index, answer: text, toolUses: [], stopReason: 'end_turn', usage: null }
      }
    }
    return yield* next(e)
  })

  on('turn.complete', async ($, e, next) => {
    const r = await next(e)
    if (e.agentId || (await $.session.surfaces()).length === 0) return r
    const tokens = (await $.session.usage()).context.tokens
    if (tokens === undefined) return r
    if (stopPending || tokens >= threshold) {
      const wasStop = stopPending
      stopPending = false
      compact($).then(
        (done) => {
          if (done) {
            hardStopOff = false
            if (wasStop) void $.prompt.submit({ text: CONTINUE })
          } else if (wasStop) hardStopOff = true
        },
        () => {
          if (wasStop) hardStopOff = true
        },
      )
    } else if (tokens >= idleThreshold) {
      idleTimer?.cancel()
      idleTimer = $.clock.after(idleMs, () => {
        idleTimer = undefined
        compact($).catch(() => {})
      })
    }
    return r
  })
}

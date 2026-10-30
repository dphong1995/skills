// Compacts the main conversation on its own: after a turn that ends at or above `threshold`,
// mid-turn at `hardLimit` (the turn is ended, compacted, then continued by a prompt), and
// `idleMinutes` after a turn that ended at or above `idleThreshold` if no turn started since.
// Subagent events and headless sessions pass through untouched.
import type { EngineInterface, Register, Timer } from 'claude-code'

const INSTRUCTIONS =
  "Keep what is needed to continue without re-reading the transcript: the user's latest request and whether it is answered; the skill or workflow in progress (name, step, round) and the paths of its plan, spec and phase files; decisions made and why; files changed and their state; commands and tests still to run with their last results; background agents and commands still running and what each is for; questions waiting for the user."

const CONTINUE =
  'The context was compacted in the middle of your last turn. Continue the step you were on. Check what is already done before redoing anything; background agents that are still running will report here.'

let stopPending = false
let hardStopOff = false
let checkAfterStop = false
let compacting = false
let mainTurns = 0
let idleTimer: Timer | undefined

const compact = ($: EngineInterface) => {
  compacting = true
  return $.session.compact({ instructions: INSTRUCTIONS }).then(
    (r) => r.skip === undefined,
    () => false,
  ).then((done) => {
    compacting = false
    if (done) hardStopOff = false
    return done
  })
}

export const register: Register = (on, options) => {
  const threshold = Number(options.threshold)
  const hardLimit = Number(options.hardLimit)
  const idleThreshold = Number(options.idleThreshold)
  const idleMs = Number(options.idleMinutes) * 60_000

  on('turn.start', async ($, e, next) => {
    if (!e.agentId) {
      idleTimer?.cancel()
      mainTurns++
    }
    return next(e)
  })

  on('turn.step', async function* ($, e, next) {
    if (e.agentId || e.index === 0 || hardStopOff || stopPending || compacting) return yield* next(e)
    const tokens = (await $.session.usage()).context.tokens ?? 0
    if (checkAfterStop) {
      checkAfterStop = false
      if (tokens >= hardLimit) hardStopOff = true
    } else if (tokens >= hardLimit && (await $.session.surfaces()).length > 0) {
      stopPending = true
      const text = `Context ${Math.round(tokens / 1000)}k ≥ ${Math.round(hardLimit / 1000)}k: dừng turn để compact, xong sẽ tự làm tiếp.`
      yield { kind: 'text', index: 0, text }
      yield { kind: 'stop', stopReason: 'end_turn', usage: null }
      return { turnId: e.turnId, index: e.index, answer: text, toolUses: [], stopReason: 'end_turn', usage: null }
    }
    return yield* next(e)
  })

  on('turn.complete', async ($, e, next) => {
    const r = await next(e)
    if (e.agentId) return r
    checkAfterStop = false
    const wasStop = stopPending
    stopPending = false
    const tokens = (await $.session.usage()).context.tokens ?? 0
    const now = wasStop || (tokens >= threshold && !e.isAborted)
    if (compacting || (!now && tokens < idleThreshold) || (await $.session.surfaces()).length === 0) return r
    if (now) {
      const turns = mainTurns
      void compact($).then((done) => {
        if (!wasStop) return
        if (!done) hardStopOff = true
        else if (turns === mainTurns) {
          checkAfterStop = true
          void $.prompt.submit({ text: CONTINUE })
        }
      })
    } else {
      idleTimer?.cancel()
      idleTimer = $.clock.after(idleMs, () => {
        idleTimer = undefined
        if (!compacting) void compact($)
      })
    }
    return r
  })
}

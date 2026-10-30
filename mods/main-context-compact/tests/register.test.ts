import { expect, mock, test } from 'claude-code/testing'
import type { Engine } from 'claude-code/testing'
import type { On, TurnStepChunk } from 'claude-code'

const K = 1000
const STOP_LINE = 'Context 400k ≥ 400k: dừng turn để compact, xong sẽ tự làm tiếp.'

type World = {
  tokens: number | undefined
  surfaces: string[]
  compactResult: 'ok' | 'skip' | 'reject'
  compacts: (string | undefined)[]
  submits: string[]
  clock: ReturnType<typeof mock.clock>
}

const world = (on: On, init: Partial<World> = {}): World => {
  const w: World = {
    tokens: 0,
    surfaces: ['terminal'],
    compactResult: 'ok',
    compacts: [],
    submits: [],
    clock: mock.clock(on),
    ...init,
  }
  on('session.surfaces', () => ({ value: w.surfaces as never }))
  on('session.usage', () => ({
    value: { startedAt: 0, context: { tokens: w.tokens, window: 1_000_000 }, rateLimits: [] },
  }))
  on('session.compact', (_, e) => {
    w.compacts.push(e.instructions)
    if (w.compactResult === 'reject') throw new Error('a turn is running')
    if (w.compactResult === 'skip') return { skip: 'vetoed' }
    return { messages: [{ role: 'user', text: 'summary', toolUses: [] }] }
  })
  on('prompt.submit', (_, e) => {
    w.submits.push(e.text)
    return { text: e.text }
  })
  on('turn.start', (_, e) => ({ turnId: e.turnId }))
  on('turn.complete', () => ({ text: '' }))
  on('turn.step', async function* (_, e) {
    yield { kind: 'text', index: 0, text: 'model' } as TurnStepChunk
    return { turnId: e.turnId, index: e.index, answer: 'model', toolUses: [], stopReason: 'end_turn', usage: null }
  })
  return w
}

const start = ($: Engine, agentId?: string) => $.turn.start({ text: 'hi', turnId: 't1', agentId })
const complete = async ($: Engine, w: World, agentId?: string) => {
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer', agentId })
  await w.clock.settle()
}
const step = async ($: Engine, index: number, agentId?: string) => {
  const chunks: TurnStepChunk[] = []
  const s = $.turn.step({ turnId: 't1', index, model: 'm', messageCount: 1, agentId })
  let n = await s.next()
  while (!n.done) {
    chunks.push(n.value)
    n = await s.next()
  }
  return { chunks, result: n.value }
}
const text = (chunks: TurnStepChunk[]) =>
  chunks.flatMap((c) => (c.kind === 'text' ? [c.text] : [])).join('')

const isInstructions = (s: string | undefined) =>
  typeof s === 'string' && s.startsWith('Keep what is needed to continue without re-reading the transcript')

test('a turn ending at 299k does not compact', async ($, on) => {
  const w = world(on, { tokens: 299 * K })
  await complete($, w)
  expect(w.compacts.length).toBe(0)
})

test('a turn ending at 300k compacts with the instructions and sends no continue', async ($, on) => {
  const w = world(on, { tokens: 300 * K })
  await complete($, w)
  expect(w.compacts.length).toBe(1)
  expect(isInstructions(w.compacts[0])).toBe(true)
  expect(w.submits.length).toBe(0)
})

test('unknown token count does nothing', async ($, on) => {
  const w = world(on, { tokens: undefined })
  await complete($, w)
  expect(w.compacts.length).toBe(0)
  expect(w.clock.now()).toBe(0)
})

test('a step at 400k stops the turn, the turn end compacts and continues', async ($, on) => {
  const w = world(on, { tokens: 400 * K })
  const { chunks, result } = await step($, 1)
  expect(text(chunks)).toBe(STOP_LINE)
  expect(chunks.some((c) => c.kind === 'stop' && c.stopReason === 'end_turn')).toBe(true)
  expect(result.stopReason).toBe('end_turn')
  expect(w.compacts.length).toBe(0)
  await complete($, w)
  expect(w.compacts.length).toBe(1)
  expect(isInstructions(w.compacts[0])).toBe(true)
  expect(w.submits.length).toBe(1)
  expect(w.submits[0]?.startsWith('The context was compacted in the middle of your last turn.')).toBe(true)
})

test('a step below 400k, or the first step, passes through', async ($, on) => {
  const w = world(on, { tokens: 399 * K })
  expect(text((await step($, 1)).chunks)).toBe('model')
  w.tokens = 450 * K
  expect(text((await step($, 0)).chunks)).toBe('model')
})

test('a second step at 400k before the turn ends does not stop twice', async ($, on) => {
  const w = world(on, { tokens: 400 * K })
  await step($, 1)
  expect(text((await step($, 2)).chunks)).toBe('model')
  await complete($, w)
  expect(w.compacts.length).toBe(1)
})

test('a refused compaction after a stop sends no continue and turns the stop off', async ($, on) => {
  const w = world(on, { tokens: 400 * K, compactResult: 'skip' })
  await step($, 1)
  await complete($, w)
  expect(w.compacts.length).toBe(1)
  expect(w.submits.length).toBe(0)
  expect(text((await step($, 1)).chunks)).toBe('model')
})

test('a rejected compaction after a stop behaves like a refused one', async ($, on) => {
  const w = world(on, { tokens: 400 * K, compactResult: 'reject' })
  await step($, 1)
  await complete($, w)
  expect(w.submits.length).toBe(0)
  expect(text((await step($, 1)).chunks)).toBe('model')
})

test('a later successful compaction turns the stop back on', async ($, on) => {
  const w = world(on, { tokens: 400 * K, compactResult: 'skip' })
  await step($, 1)
  await complete($, w)
  w.compactResult = 'ok'
  await complete($, w)
  expect(w.compacts.length).toBe(2)
  expect(w.submits.length).toBe(0)
  expect(text((await step($, 1)).chunks)).toBe(STOP_LINE)
})

test('events with an agentId are left alone', async ($, on) => {
  const w = world(on, { tokens: 450 * K })
  expect(text((await step($, 1, 'a1')).chunks)).toBe('model')
  await complete($, w, 'a1')
  expect(w.compacts.length).toBe(0)
  w.tokens = 250 * K
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer', agentId: 'a1' })
  await w.clock.advance(60 * 60 * 1000)
  expect(w.compacts.length).toBe(0)
})

test('a headless session (no surfaces) is left alone', async ($, on) => {
  const w = world(on, { tokens: 450 * K, surfaces: [] })
  expect(text((await step($, 1)).chunks)).toBe('model')
  await complete($, w)
  w.tokens = 250 * K
  await complete($, w)
  await w.clock.advance(60 * 60 * 1000)
  expect(w.compacts.length).toBe(0)
})

test('idle at 250k compacts after 55 minutes, with no continue', async ($, on) => {
  const w = world(on, { tokens: 250 * K })
  await complete($, w)
  expect(w.compacts.length).toBe(0)
  await w.clock.advance(54 * 60 * 1000)
  expect(w.compacts.length).toBe(0)
  await w.clock.advance(60 * 1000)
  expect(w.compacts.length).toBe(1)
  expect(isInstructions(w.compacts[0])).toBe(true)
  expect(w.submits.length).toBe(0)
})

test('a turn starting before minute 55 cancels the idle compact', async ($, on) => {
  const w = world(on, { tokens: 250 * K })
  await complete($, w)
  await w.clock.advance(30 * 60 * 1000)
  await start($)
  await w.clock.advance(60 * 60 * 1000)
  expect(w.compacts.length).toBe(0)
})

test('150k schedules nothing', async ($, on) => {
  const w = world(on, { tokens: 150 * K })
  await complete($, w)
  await w.clock.advance(120 * 60 * 1000)
  expect(w.compacts.length).toBe(0)
})

test('a subagent turn starting does not cancel the idle compact', async ($, on) => {
  const w = world(on, { tokens: 250 * K })
  await complete($, w)
  await start($, 'a1')
  await w.clock.advance(55 * 60 * 1000)
  expect(w.compacts.length).toBe(1)
})

test('options change the thresholds', async ($, on) => {
  const w = world(on, { tokens: 60 * K })
  await complete($, w)
  expect(w.compacts.length).toBe(0)
})

test('options change the thresholds: 60k compacts when the threshold is 60k', { options: { threshold: 60 * K } }, async ($, on) => {
  const w = world(on, { tokens: 60 * K })
  await complete($, w)
  expect(w.compacts.length).toBe(1)
})

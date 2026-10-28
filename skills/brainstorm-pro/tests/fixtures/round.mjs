// The sample round: paths are resolved from this file, so the JSON is written at run time.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const TESTS = dirname(HERE);

export const round = () => ({
  root: TESTS,
  topic: 'Fixture round',
  decided: ['Use `fixtures/code.ts` as the sample', 'Keep the page one file'],
  questions: [
    {
      id: 'q01', n: 1, stage: 'Design', title: 'Which adder?', reversible: true, reversibleWhy: 'Only a local helper.',
      why: 'The module needs one before the next step.',
      recommendation: 'B is the smallest change.',
      context: [
        { t: 'p', text: 'See `fixtures/code.ts:2-4` for the current adder.' },
        { t: 'code', file: join(HERE, 'code.ts'), lines: [2, 4] },
        { t: 'diff', label: 'Before and after', before: { text: 'const a = 1;\nconst b = 2;', lang: 'ts' }, after: { text: 'const a = 1;\nconst c = 3;\nconst b = 2;', lang: 'ts' } },
        { t: 'mermaid', label: 'Flow', text: 'flowchart LR\n  A --> B' },
        { t: 'img', file: 'fixtures/img.png', label: 'Tiny image' },
        { t: 'mockup', label: 'Mockup', html: '<button class="btn">Mock</button>' },
        { t: 'table', head: ['Name', 'Cost'], rows: [['alpha', '`1`'], ['beta', '**2**']] },
        { t: 'note', text: 'Check the numbers before choosing.' },
      ],
      options: [
        { id: 'a', label: 'Plain add', means: 'Add two numbers.', example: 'add(1, 2) is 3', pros: ['Simple'], cons: ['Slow'], effort: 'small' },
        { id: 'b', label: 'Checked add', means: 'Add with a guard, see `fixtures/code.ts:1-2`.', recommended: true, example: 'add(1, NaN) throws', pros: ['Safe', 'Clear'], cons: ['More code'], effort: 'medium' },
        { id: 'c', label: 'Lazy add', means: 'Add on demand.', example: 'add(1)(2)', pros: ['Flexible'], cons: ['Odd'], effort: 'large' },
      ],
    },
    {
      id: 'q02', n: 2, title: 'Which extras?', multi: true,
      options: [
        { id: 'x', label: 'Extra X', means: 'The first extra.' },
        { id: 'y', label: 'Extra Y', means: 'The second extra.' },
      ],
    },
  ],
});

// Writes the round to <dir>/round.json (after `mutate`, for the error cases) and returns the path.
export const writeRound = (dir, mutate) => {
  const data = round();
  if (mutate) mutate(data);
  const path = join(dir, 'round.json');
  writeFileSync(path, JSON.stringify(data, null, 1));
  return path;
};

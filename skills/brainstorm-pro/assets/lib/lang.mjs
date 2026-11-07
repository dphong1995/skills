// Which language the pages use: --lang wins, else the `language` setting of ${CLAUDE_HOME:-~/.claude}/settings.json; Vietnamese only when it says so, else English.
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const fold = (s) => s.normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/đ/g, 'd');

export const pageLang = ({ flag, env = process.env } = {}) => {
  if (flag === 'vi' || flag === 'en') return flag;
  try {
    const setting = JSON.parse(readFileSync(join(env.CLAUDE_HOME || join(homedir(), '.claude'), 'settings.json'), 'utf8')).language;
    if (typeof setting !== 'string') return 'en';
    const l = fold(setting.toLowerCase());
    return l === 'vi' || l.includes('viet') ? 'vi' : 'en';
  } catch (e) { return 'en'; }
};

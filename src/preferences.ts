import type { Provider } from './types';
type Preferences = {
  provider: Provider;
  models: Partial<Record<Provider, { model: string; effort: string }>>;
};
const key = 'hns.preferences.v1';
export function preferences(): Preferences {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(key) || '{}');
    if (!raw || typeof raw !== 'object') throw new Error();
    const value = raw as Record<string, unknown>;
    const models: Preferences['models'] = {};
    for (const p of ['chatgpt', 'grok'] as const) {
      const saved = (value.models as Preferences['models'] | undefined)?.[p];
      if (typeof saved?.model === 'string' && typeof saved.effort === 'string')
        models[p] = saved;
    }
    return { provider: value.provider === 'grok' ? 'grok' : 'chatgpt', models };
  } catch {
    return { provider: 'chatgpt', models: {} };
  }
}
export function savePreferences(
  provider: Provider,
  model?: string,
  effort = '',
) {
  const value = preferences();
  value.provider = provider;
  if (model) value.models[provider] = { model, effort };
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* Preferences are optional when storage is unavailable. */
  }
}

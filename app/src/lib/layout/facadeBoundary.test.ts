import { describe, expect, it } from 'vitest';

const sources = import.meta.glob(
  ['/src/routes/**/*.{ts,svelte}', '/src/lib/components/**/*.{ts,svelte}'],
  { eager: true, query: '?raw', import: 'default' },
) as Record<string, string>;

const editStores = new Set([
  'bowties', 'layout', 'bowtieMetadata', 'configChanges',
  'channels', 'facilities', 'connectorSelections', 'pendingChanges',
]);

function violations(file: string, source: string): string[] {
  if (/(?:\.test|\.integration\.test)\.ts$|(?:TestSupport|testSupport)\.ts$/.test(file)) return [];
  const imports = /\b(?:import\s*(?:\(\s*['"`]([^'"`]+)['"`]\s*\)|(?:[^;'"()]*?\sfrom\s*)?['"]([^'"]+)['"])|export\s+(?:[^;'"()]*?\sfrom\s*)['"]([^'"]+)['"])/g;
  return Array.from(source.matchAll(imports), (match) => match[1] ?? match[2] ?? match[3])
    .filter((specifier) => {
      const resolved = specifier.startsWith('$lib/')
        ? `/src/lib/${specifier.slice(5)}`
        : specifier.startsWith('.') ? new URL(specifier, `http://facade-boundary${file}`).pathname : specifier;
      const store = /^\/src\/lib\/stores\/([^/]+?)(?:\.svelte)?(?:\.ts)?$/.exec(resolved);
      return store !== null && editStores.has(store[1]);
    });
}

describe('layout facade boundary', () => {
  it('rejects direct edit-store imports from production routes and components', () => {
    for (const store of editStores) {
      const source = `import { x } from '$lib/stores/${store}.svelte';\n` +
        `import type { X } from '../stores/${store}.svelte';\n` +
        `const x = import('$lib/stores/${store}.svelte');\n` +
        `export { x } from '../stores/${store}.svelte';\n` +
        'const y = import(`$lib/stores/' + store + '.svelte`);';
      expect(violations('/src/lib/components/example.ts', source)).toHaveLength(5);
      expect(violations('/src/lib/components/example.test.ts', source)).toEqual([]);
      expect(violations('/src/lib/components/example.integration.test.ts', source)).toEqual([]);
      expect(violations('/src/lib/components/exampleTestSupport.ts', source)).toEqual([]);
      expect(violations('/src/lib/components/exampletestSupport.ts', source)).toEqual([]);
    }
    const found = Object.entries(sources).flatMap(([file, source]) =>
      violations(file, source).map((specifier) => `${file}: ${specifier}`),
    );
    expect(found).toEqual([]);
  });
});

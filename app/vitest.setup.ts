import '@testing-library/jest-dom';

// jsdom does not implement window.matchMedia. Components that import
// `@zerodevx/svelte-toast` (transitively via svelte/motion) crash on
// module load without this polyfill.
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}

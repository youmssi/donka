import '@testing-library/jest-dom/vitest';

// jsdom has no ResizeObserver; Radix primitives (checkbox, select) measure themselves with it.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub;

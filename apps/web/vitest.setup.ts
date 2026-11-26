import '@testing-library/jest-dom/vitest';

// jsdom has no ResizeObserver; Radix primitives (checkbox, select) measure themselves with it.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub;

// jsdom does not scroll; cmdk (the command menu and comboboxes) scrolls the active item into view.
Element.prototype.scrollIntoView ??= function scrollIntoView() {};

// jsdom has no pointer capture; Radix Select checks it when its trigger is pressed.
Element.prototype.hasPointerCapture ??= function hasPointerCapture() {
  return false;
};
Element.prototype.releasePointerCapture ??= function releasePointerCapture() {};

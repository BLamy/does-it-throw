// Mock console.log for WASM module
// The WASM module expects console.log to be available globally
// @ts-ignore
global.console = global.console || {};
// @ts-ignore
global.console.log = global.console.log || (() => {});
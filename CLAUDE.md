# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

This is "What Does It Throw?" (WDIT) - a VSCode extension and Language Server Protocol (LSP) implementation that finds throw statements in JavaScript, TypeScript, JSX, and TSX files. The core analysis logic is written in Rust and compiled to WebAssembly for performance.

## Architecture

This is a multi-component project with several interconnected parts:

### Core Components
- **Rust Core** (`crates/what-does-it-throw/`): Main analysis engine using SWC for AST parsing
- **WASM Module** (`crates/what-does-it-throw-wasm/`): WebAssembly bindings for the Rust core
- **LSP Server** (`server/`): TypeScript-based Language Server Protocol implementation
- **VSCode Client** (`client/`): VSCode extension client that communicates with the LSP server
- **JetBrains Plugin** (`jetbrains/`): Kotlin-based IntelliJ/JetBrains IDE plugin

### Key Architecture Points
- Rust code is compiled to WASM for cross-platform compatibility
- LSP server loads and uses the WASM module for analysis
- Cross-file analysis tracks imports/exports to detect throwing functions
- Client-server architecture allows the heavy analysis to run in a separate process
- The build system uses a custom TypeScript orchestrator (`build.ts`) that handles both WASM compilation and TypeScript bundling

## Development Setup

### Prerequisites
- **Bun** (>1.0.0) - Primary package manager and build tool
- **Node.js** (>20) - Runtime for TypeScript components
- **Rust** (latest stable) - For core analysis engine
- **wasm-pack** - For compiling Rust to WebAssembly
- **Java 11+** - For JetBrains plugin development (optional)

### Installation
```bash
# Install dependencies
bun install

# Install Rust toolchain if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install wasm-pack
cargo install wasm-pack
```

## Build Commands

The project uses a custom build system orchestrated by `build.ts`:

### Core Build Commands
- `bun run build` - Full build (WASM + TypeScript bundling)
- `bun run build:wasm` - Build only the Rust WASM modules
- `bun run build:ts` - Build only TypeScript components
- `bun run dev` - Development mode with file watching
- `bun run watch` - Watch mode for development

### VSCode Extension
- `bun run vscode:package` - Create .vsix package for installation
- `bun run vscode:publish` - Publish to VSCode marketplace
- `bun run vscode:release` - Release to marketplace using vsce
- `bun run vscode:prepublish` - Prepare for publishing (runs compile)

### Testing
- `bun run test` - Run all tests using Vitest
- `bun run test:run` - Run tests once without watch mode
- `bun run test:watch` - Run tests in watch mode
- `bun run test:ui` - Run tests with Vitest UI
- `cargo test` - Run Rust tests directly

### Single Test Execution
- For TypeScript tests: `bun test tests/specific-test.test.ts`
- For Rust tests: `cargo test test_name`
- With Vitest: `vitest run specific-test`

### Code Quality
- `bun run format` - Format code using Biome
- `biome format .` - Direct Biome formatting

### Publishing
- `bun run pack:server` - Create npm package for LSP server
- `bun run publish:server` - Publish LSP server to npm

## Key Files and Directories

### Rust Core (`crates/what-does-it-throw/src/`)
- `lib.rs` - Main entry point and public API
- `throw_finder.rs` - AST visitor for finding throw statements
- `call_finder.rs` - Finds function calls that may throw
- `import_usage_finder.rs` - Tracks imports/exports for cross-file analysis
- `function_finder.rs` - Identifies function declarations and expressions
- `try_catch_finder.rs` - Analyzes try-catch blocks for exhaustive error handling
- `fixtures/` - Test files for various JavaScript/TypeScript patterns

### TypeScript Components
- `server/src/server.ts` - LSP server implementation
- `client/src/extension.ts` - VSCode extension client
- Both use the compiled WASM module for analysis

### Build System
- `build.ts` - Custom build orchestrator using Bun
- Handles WASM compilation via `wasm-pack`
- TypeScript bundling via esbuild
- Automatic file copying and watching

### Testing
- `tests/` - Integration tests for the extension
- `vitest.config.ts` - Vitest configuration
- Test utilities in `tests/test-utils.ts`

## Development Workflow

### Making Changes

1. **Rust Changes**: Modify files in `crates/`, then run `bun run build:wasm`
2. **TypeScript Changes**: Modify `server/` or `client/`, then run `bun run build:ts`
3. **Full Rebuild**: Run `bun run build` after any changes
4. **Testing**: Use fixture files in `crates/what-does-it-throw/src/fixtures/` for testing

### Extension Development
1. Build the extension: `bun run build`
2. Package for testing: `bun run vscode:package`
3. Install in VSCode/Cursor: Extensions → Install from VSIX

### Debug Tips
- Use `console.log` in TypeScript server code
- Rust debug output goes to LSP server logs
- VSCode Developer Tools → Help → Toggle Developer Tools for client debugging
- Extension logs: Output panel → "What Does It Throw?"

## Configuration

The extension supports these settings:
- `whatDoesItThrow.enabled` - Enable/disable the extension
- `whatDoesItThrow.throwStatementSeverity` - Severity for throw statements (Error/Warning/Information/Hint)
- `whatDoesItThrow.functionThrowSeverity` - Severity for functions that throw
- `whatDoesItThrow.callToThrowSeverity` - Severity for calls to throwing functions
- `whatDoesItThrow.callToImportedThrowSeverity` - Severity for calls to imported throwing functions
- `whatDoesItThrow.maxNumberOfProblems` - Maximum diagnostics (default: 100)
- `whatDoesItThrow.includeTryStatementThrows` - Include throws inside try blocks (default: false)
- `whatDoesItThrow.ignoreStatements` - Comment patterns to ignore (default: ["@it-throws", "@what-does-it-throw-ignore"])
- `whatDoesItThrow.trace.server` - LSP trace level (off/messages/verbose)

## Publishing

### VSCode Extension
- Managed by `vsce` (Visual Studio Code Extension manager)
- Version bumping handled by release-please automation
- Published to VSCode marketplace under publisher `michaelangeloio`

### NPM Packages
- Server component published as `what-does-it-throw-lsp`
- Can be used independently in other LSP clients
- Binary available at `what-does-it-throw-lsp` command

## Workspace Structure

The project uses npm workspaces:
- Root workspace manages overall dependencies
- `client/` - VSCode extension client workspace
- `server/` - LSP server workspace (publishable separately)

## Cross-Platform Considerations

- WASM ensures the core analysis works everywhere Node.js runs
- JetBrains plugin provides IDE support beyond VSCode
- All file paths use forward slashes internally for consistency
- WASM module is compiled with target `nodejs` for compatibility

## Performance Notes

- WASM compilation provides near-native performance for AST analysis
- Cross-file analysis is cached to avoid recomputation
- Large codebases benefit from the Rust implementation's speed
- File watching minimizes rebuild times during development
- Build uses esbuild for fast TypeScript compilation

## Common Issues

1. **WASM Build Failures**: Ensure `wasm-pack` is installed and Rust toolchain is current
2. **TypeScript Errors**: Run `bun run build:ts` to see detailed compilation errors
3. **Extension Not Loading**: Check that WASM files are present in `server/out/` directory after build
4. **Performance Issues**: Large projects may need increased Node.js memory limits
5. **Test Failures**: Ensure WASM is built before running tests (`bun run build:wasm`)

## Important Notes

- Use bun for package management and running npm scripts
- Always build WASM before TypeScript to ensure the latest analysis engine is bundled
- The project uses Biome for formatting, not Prettier or ESLint
- Tests use Vitest, not Jest
- The main branch is `main` (not `master`)
- when making changes to rust you must run the test for rust
- when making changes to typescript you must run vitest
- Each time you implement a feature and all test pass make sure you version bump the vscode plugin and build a new vsix then commit the changes and conform to @commitlint.config.js
- DO NOT commit if all test are not passing or if you have any outstanding task left that you did not finish
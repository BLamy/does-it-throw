import {
  DidChangeConfigurationNotification,
  InitializeParams,
  InitializeResult,
  ProposedFeatures,
  TextDocumentSyncKind,
  TextDocuments,
  createConnection,
  CodeActionKind,
  CodeAction,
  CodeActionParams,
  Range,
  Position
} from 'vscode-languageserver/node'

import { access, constants, readFile } from 'fs/promises'
import { TextDocument } from 'vscode-languageserver-textdocument'
import { InputData, ParseResult, parse_js } from './rust/what_does_it_throw_wasm'
import path = require('path')
import { inspect } from 'util'

const connection = createConnection(ProposedFeatures.all)

const documents: TextDocuments<TextDocument> = new TextDocuments(TextDocument)
let hasConfigurationCapability = false
let hasWorkspaceFolderCapability = false

connection.onInitialize((params: InitializeParams) => {
  const capabilities = params.capabilities

  // Does the client support the `workspace/configuration` request?
  // If not, we fall back using global settings.
  hasConfigurationCapability = !!(capabilities.workspace && !!capabilities.workspace.configuration)
  hasWorkspaceFolderCapability = !!(capabilities.workspace && !!capabilities.workspace.workspaceFolders)

  const result: InitializeResult = {
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Incremental,
      codeActionProvider: {
        codeActionKinds: [CodeActionKind.QuickFix]
      }
    }
  }
  if (hasWorkspaceFolderCapability) {
    result.capabilities.workspace = {
      workspaceFolders: {
        supported: true
      }
    }
  }
  return result
})

connection.onInitialized(() => {
  if (hasConfigurationCapability) {
    // Register for all configuration changes.
    connection.client.register(DidChangeConfigurationNotification.type, undefined)
  }
  if (hasWorkspaceFolderCapability) {
    connection.workspace.onDidChangeWorkspaceFolders((_event) => {
      connection.console.log(`Workspace folder change event received. ${JSON.stringify(_event)}`)
    })
  }
})

type DiagnosticSeverity = 'Error' | 'Warning' | 'Information' | 'Hint'

// The server settings
interface Settings {
  maxNumberOfProblems: number
  throwStatementSeverity: DiagnosticSeverity
  functionThrowSeverity: DiagnosticSeverity
  callToThrowSeverity: DiagnosticSeverity
  callToImportedThrowSeverity: DiagnosticSeverity
  includeTryStatementThrows: boolean
  ignoreStatements: string[]
}

// The global settings, used when the `workspace/configuration` request is not supported by the client.
// Please note that this is not the case when using this server with the client provided in this example
// but could happen with other clients.
const defaultSettings: Settings = {
  maxNumberOfProblems: 1000000,
  throwStatementSeverity: 'Hint',
  functionThrowSeverity: 'Hint',
  callToThrowSeverity: 'Hint',
  callToImportedThrowSeverity: 'Hint',
  includeTryStatementThrows: false,
  ignoreStatements: ['@it-throws', '@what-does-it-throw-ignore']
}
// 👆 very unlikely someone will have more than 1 million throw statements, lol
// if you do, might want to rethink your code?
let globalSettings: Settings = defaultSettings

// Cache the settings of all open documents
const documentSettings: Map<string, Thenable<Settings>> = new Map()

connection.onDidChangeConfiguration((change) => {
  if (hasConfigurationCapability) {
    // Reset all cached document settings
    documentSettings.clear()
  } else {
    globalSettings = <Settings>(change.settings.whatDoesItThrow || defaultSettings)
  }

  // Revalidate all open text documents
  // biome-ignore lint/complexity/noForEach: original vscode-languageserver code
  documents.all().forEach(validateTextDocument)
})

function getDocumentSettings(resource: string): Thenable<Settings> {
  if (!hasConfigurationCapability) {
    connection.console.info(`does not have config capability, using global settings: ${JSON.stringify(globalSettings)}`)
    return Promise.resolve(globalSettings)
  }
  let result = documentSettings.get(resource)
  if (!result) {
    result = connection.workspace.getConfiguration({
      scopeUri: resource,
      section: 'whatDoesItThrow'
    })
    documentSettings.set(resource, result)
  }
  return result
}

// Only keep settings for open documents
documents.onDidClose((e) => {
  documentSettings.delete(e.document.uri)
})

// The content of a text document has changed. This event is emitted
// when the text document first opened or when its content has changed.
documents.onDidChangeContent(async (change) => {
  validateTextDocument(change.document)
})

documents.onDidSave((change) => {
  validateTextDocument(change.document)
})

const _checkAccessOnFile = async (file: string) => {
  try {
    await access(file, constants.R_OK)
    return Promise.resolve(file)
  } catch (e) {
    return Promise.reject(e)
  }
}

const findFirstFileThatExists = async (uri: string, relative_import: string) => {
  const isTs = uri.endsWith('.ts') || uri.endsWith('.tsx')
  const baseUri = `${path.resolve(path.dirname(uri.replace('file://', '')), relative_import)}`
  let files = Array(4)
  if (isTs) {
    files = [`${baseUri}.ts`, `${baseUri}.tsx`, `${baseUri}.js`, `${baseUri}.jsx`]
  } else {
    files = [`${baseUri}.js`, `${baseUri}.jsx`, `${baseUri}.ts`, `${baseUri}.tsx`]
  }
  return Promise.any(files.map(_checkAccessOnFile))
}

async function validateTextDocument(textDocument: TextDocument): Promise<void> {
  let settings = await getDocumentSettings(textDocument.uri)
  if (!settings) {
    // this should never happen, but just in case
    connection.console.warn(`No settings found for ${textDocument.uri}, using defaults`)
    settings = defaultSettings
  }
  try {
    const opts = {
      file_content: textDocument.getText(),
      function_throw_severity: settings?.functionThrowSeverity ?? defaultSettings.functionThrowSeverity,
      throw_statement_severity: settings?.throwStatementSeverity ?? defaultSettings.throwStatementSeverity,
      call_to_imported_throw_severity:
        settings?.callToImportedThrowSeverity ?? defaultSettings.callToImportedThrowSeverity,
      call_to_throw_severity: settings?.callToThrowSeverity ?? defaultSettings.callToThrowSeverity,
      include_try_statement_throws: settings?.includeTryStatementThrows ?? defaultSettings.includeTryStatementThrows,
      ignore_statements: settings?.ignoreStatements ?? defaultSettings.ignoreStatements
    } satisfies InputData
    const analysis = parse_js(opts) as ParseResult

    if (analysis.relative_imports.length > 0) {
      const seenImportedThrowIds = new Set<string>();
      const filePromises = analysis.relative_imports.map(async (relative_import) => {
        try {
          const file = await findFirstFileThatExists(textDocument.uri, relative_import)
          return {
            fileContent: await readFile(file, 'utf-8'),
            fileUri: file
          }
        } catch (e) {
          connection.console.log(`Error reading file ${inspect(e)}`)
          return undefined
        }
      })
      const files = (await Promise.all(filePromises)).filter((file) => !!file)
      const analysisArr = files.map((file) => {
        if (!file) {
          return undefined
        }
        const opts = {
          file_content: file.fileContent,
        } satisfies InputData
        return parse_js(opts) as ParseResult
      })
      // TODO - this is a bit of a mess, but it works for now.
      // The original analysis is the one that has the throw statements Map()
      // We get the get the throw_ids from the imported analysis and then
      // check the original analysis for existing throw_ids.
      // This allows to to get the diagnostics from the imported analysis (one level deep for now)
      for (const import_analysis of analysisArr) {
        if (!import_analysis) {
          return
        }
        if (import_analysis.throw_ids.length) {
          for (const throw_id of import_analysis.throw_ids) {
            if (seenImportedThrowIds.has(throw_id)) continue;
            seenImportedThrowIds.add(throw_id);
            const newDiagnostics = analysis.imported_identifiers_diagnostics.find(item => item.id === throw_id)
            if (newDiagnostics?.diagnostics?.length) {
              analysis.diagnostics.push(...newDiagnostics.diagnostics)
            }
          }
        }
      }
    }
    // Convert diagnostics to 0-based line indices for LSP/VS Code
    const zeroBasedDiagnosticsRaw = analysis.diagnostics.map((d) => {
      try {
        const startLine = Math.max(0, (d.range?.start?.line ?? 0) - 1)
        const endLine = Math.max(0, (d.range?.end?.line ?? startLine) - 1)
        return {
          ...d,
          range: {
            ...d.range,
            start: { ...d.range.start, line: startLine },
            end: { ...d.range.end, line: endLine }
          }
        }
      } catch {
        return d
      }
    })

    // Deduplicate diagnostics by range and message to avoid spurious duplicates
    const seen = new Set<string>()
    const zeroBasedDiagnostics = zeroBasedDiagnosticsRaw.filter(d => {
      const key = `${d.message}|${d.range?.start?.line}:${d.range?.start?.character}-${d.range?.end?.line}:${d.range?.end?.character}|${d.severity}|${d.code}`
      if (seen.has(key)) return false
      seen.add(key)
      return true
    })

    connection.sendDiagnostics({
      uri: textDocument.uri,
      diagnostics: zeroBasedDiagnostics
    })
  } catch (e) {
    console.log(e)
    connection.console.error(`Error parsing file ${textDocument.uri}`)
    connection.console.error(`settings are: ${JSON.stringify(settings)}`)
    connection.console.error(`Error: ${e instanceof Error ? e.message : JSON.stringify(e)} error`)
    connection.sendDiagnostics({ uri: textDocument.uri, diagnostics: [] })
  }
}

connection.onDidChangeWatchedFiles((_change) => {
  // Monitored files have change in VSCode
  connection.console.log(`We received an file change event ${_change}, not implemented yet`)
})

// Handle code actions for quick fixes
connection.onCodeAction((params: CodeActionParams): CodeAction[] => {
  const textDocument = documents.get(params.textDocument.uri)
  if (!textDocument) {
    return []
  }

  const codeActions: CodeAction[] = []

  // Check if there are diagnostics in the range that we can fix
  for (const diagnostic of params.context.diagnostics) {
    if (diagnostic.source === 'Does it Throw?' && 
        diagnostic.message.includes('Exhaustive catch is missing handlers for:')) {
      
      // Extract missing error types from the diagnostic message
      const match = diagnostic.message.match(/missing handlers for: ([^.]+)/)
      if (match) {
        const missingTypes = match[1].split(', ').map(type => type.trim())
        
        // Get the catch block content to determine where to insert the handlers
        const catchRange = diagnostic.range
        const catchLine = textDocument.getText(Range.create(
          Position.create(catchRange.start.line, 0),
          Position.create(catchRange.start.line + 10, 0) // Read a few lines to find the catch block
        ))
        
        // Find the position to insert instanceof checks (after the opening brace)
        const insertPositionStart = findInsertPositionStart(textDocument, catchRange)
        const insertPositionEnd = findInsertPositionEnd(textDocument, catchRange)
        
        if (insertPositionStart) {
          // Create code action to add instanceof handlers
          const handlersText = missingTypes.map(errorType => 
            `    if (e instanceof ${errorType}) {\n      // Handle ${errorType}\n      console.error('${errorType}:', e.message);\n      return null;\n    }`
          ).join(' else ') + '\n'
          
          const addHandlersAction: CodeAction = {
            title: `Add instanceof handlers for ${missingTypes.join(', ')}`,
            kind: CodeActionKind.QuickFix,
            diagnostics: [diagnostic],
            edit: {
              changes: {
                [params.textDocument.uri]: [{
                  range: Range.create(insertPositionStart, insertPositionStart),
                  newText: handlersText
                }]
              }
            }
          }
          codeActions.push(addHandlersAction)
        }
        
        if (insertPositionEnd) {
          // Create code action to add escape hatch at the end
          const escapeHatchAction: CodeAction = {
            title: "Add 'throw e' as escape hatch",
            kind: CodeActionKind.QuickFix,
            diagnostics: [diagnostic],
            edit: {
              changes: {
                [params.textDocument.uri]: [{
                  range: Range.create(insertPositionEnd, insertPositionEnd),
                  newText: '    // Escape hatch for unhandled errors\n    throw e;\n'
                }]
              }
            }
          }
          codeActions.push(escapeHatchAction)
        }
      }
    } else if (diagnostic.source === 'Does it Throw?' && 
               (diagnostic.message.match(/^Function .+ may throw(?:: .+)?$/) || diagnostic.message.startsWith('Anonymous function may throw'))) {
      
      // Handle function-level diagnostics - add JSDoc @throws or convert anonymous callback
      const anon = diagnostic.message.startsWith('Anonymous function may throw')
      const extracted = diagnostic.message.match(/^Function (.+) may throw(?:: (.+))?$/)
      const functionName = extracted && extracted[1] ? extracted[1] : '<anonymous>'
      const typesPart = extracted && extracted[2] ? extracted[2] : ''
      const errorTypes = typesPart.length > 0 ? typesPart.split(', ').map((t: string) => t.trim()) : ['Error']

      // Suggest adding JSDoc before the function/callback
      const insertPosition = findJSDocInsertPosition(textDocument, diagnostic.range)
      if (insertPosition) {
        const jsdocLines = [
          '/**',
          ...errorTypes.map((errorType: string) => ` * @throws {${errorType}}`),
          ' */'
        ]
        const jsdocText = jsdocLines.join('\n') + '\n'

        const addJSDocAction: CodeAction = {
          title: anon ? `Annotate anonymous function with @throws` : `Add JSDoc @throws for ${errorTypes.join(', ')}`,
          kind: CodeActionKind.QuickFix,
          diagnostics: [diagnostic],
          edit: {
            changes: {
              [params.textDocument.uri]: [{
                range: Range.create(insertPosition, insertPosition),
                newText: jsdocText
              }]
            }
          }
        }
        codeActions.push(addJSDocAction)
      }

      // If anonymous callback, suggest converting to a named function with JSDoc
      if (anon) {
        const range = diagnostic.range
        const callbackText = textDocument.getText(range)
        // Heuristic: if it looks like an arrow function, replace `() => { ... }` with `function callbackName() { ... }`
        // We'll choose a friendly default name but keep it easy to rename by user.
        const replacement = callbackText
          .replace(/\(\s*([^)]*)\s*\)\s*=>\s*\{/, '/**\n * @throws {Error}\n */\nfunction callbackThrows($1) {')
          .replace(/\)\s*=>\s*([^\{][^;]*);?$/, ') { return $1; }')

        const convertAction: CodeAction = {
          title: 'Convert to named function with @throws',
          kind: CodeActionKind.QuickFix,
          diagnostics: [diagnostic],
          edit: {
            changes: {
              [params.textDocument.uri]: [{
                range,
                newText: replacement
              }]
            }
          }
        }
        codeActions.push(convertAction)
      }
    } else if (diagnostic.source === 'Does it Throw?' && 
               diagnostic.message === 'Function call may throw: {Error}.') {
      
      // Handle function call diagnostics - add @it-throws comment
      const insertPosition = findCommentInsertPosition(textDocument, diagnostic.range)
      
      if (insertPosition) {
        const addCommentAction: CodeAction = {
          title: "Add @it-throws comment to suppress warning",
          kind: CodeActionKind.QuickFix,
          diagnostics: [diagnostic],
          edit: {
            changes: {
              [params.textDocument.uri]: [{
                range: Range.create(insertPosition, insertPosition),
                newText: getIndentedComment(textDocument, diagnostic.range)
              }]
            }
          }
        }
        codeActions.push(addCommentAction)
      }
    } else if (diagnostic.source === 'Does it Throw?' && 
               diagnostic.message.includes('Unused @it-throws comment')) {
      
      // Handle unused @it-throws comment diagnostics - offer to remove them
      const removeCommentAction: CodeAction = {
        title: "Remove unused @it-throws comment",
        kind: CodeActionKind.QuickFix,
        diagnostics: [diagnostic],
        edit: {
          changes: {
            [params.textDocument.uri]: [{
              range: Range.create(
                Position.create(diagnostic.range.start.line, 0),
                Position.create(diagnostic.range.start.line + 1, 0)
              ),
              newText: ''
            }]
          }
        }
      }
      codeActions.push(removeCommentAction)
    }
  }

  return codeActions
})

// Helper function to find the position to insert code at the start of a catch block
function findInsertPositionStart(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  // Start from the diagnostic line and look for the catch block structure
  const startLine = diagnosticRange.start.line
  
  // Look for the opening brace of the catch block
  for (let line = startLine; line < Math.min(startLine + 10, textDocument.lineCount); line++) {
    const lineText = textDocument.getText(Range.create(
      Position.create(line, 0),
      Position.create(line + 1, 0)
    ))
    
    // Find catch (e) { pattern
    const catchMatch = lineText.match(/catch\s*\([^)]+\)\s*\{/)
    if (catchMatch) {
      // Insert after the opening brace, on the next line with proper indentation
      return Position.create(line + 1, 0)
    }
  }
  
  return null
}

// Helper function to find the position to insert code at the end of a catch block (before closing brace)
function findInsertPositionEnd(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  // Start from the diagnostic line and look for the catch block structure
  const startLine = diagnosticRange.start.line
  let braceCount = 0
  let foundCatchStart = false
  
  // Look for the catch block and track braces to find the end
  for (let line = startLine; line < Math.min(startLine + 20, textDocument.lineCount); line++) {
    const lineText = textDocument.getText(Range.create(
      Position.create(line, 0),
      Position.create(line + 1, 0)
    ))
    
    // Find catch (e) { pattern
    if (!foundCatchStart && lineText.match(/catch\s*\([^)]+\)\s*\{/)) {
      foundCatchStart = true
      braceCount = 1
      continue
    }
    
    if (foundCatchStart) {
      // Count braces to find the matching closing brace
      for (const char of lineText) {
        if (char === '{') braceCount++
        if (char === '}') braceCount--
        
        if (braceCount === 0) {
          // Found the closing brace, insert before it
          return Position.create(line, 0)
        }
      }
    }
  }
  
  return null
}

// Helper function to find the position to insert JSDoc before a function
function findJSDocInsertPosition(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  // The diagnostic range should cover the function declaration
  // We want to insert JSDoc right before the function starts
  const startLine = diagnosticRange.start.line
  
  // Look backwards from the diagnostic line to find any existing comments or the actual start
  let insertLine = startLine
  for (let line = startLine - 1; line >= 0; line--) {
    const lineText = textDocument.getText(Range.create(
      Position.create(line, 0),
      Position.create(line + 1, 0)
    )).trim()
    
    // If we hit a non-empty line that's not a comment, stop here
    if (lineText && !lineText.startsWith('//') && !lineText.startsWith('/*') && !lineText.startsWith('*')) {
      insertLine = line + 1
      break
    }
    
    // If we hit the beginning of the file, insert at the start
    if (line === 0) {
      insertLine = 0
      break
    }
  }
  
  return Position.create(insertLine, 0)
}

// Helper function to find the position to insert @it-throws comment before a function call
function findCommentInsertPosition(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  // The diagnostic range covers the function call
  // We want to insert the comment on the line directly above
  const callLine = diagnosticRange.start.line
  
  // Get the indentation of the current line to match it
  const currentLineText = textDocument.getText(Range.create(
    Position.create(callLine, 0),
    Position.create(callLine + 1, 0)
  ))
  
  // Extract indentation (spaces or tabs at the start of the line)
  const indentMatch = currentLineText.match(/^(\s*)/)
  const indent = indentMatch ? indentMatch[1] : ''
  
  return Position.create(callLine, 0)
}

// Helper function to get properly indented @it-throws comment
function getIndentedComment(textDocument: TextDocument, diagnosticRange: Range): string {
  const callLine = diagnosticRange.start.line
  
  // Get the indentation of the current line to match it
  const currentLineText = textDocument.getText(Range.create(
    Position.create(callLine, 0),
    Position.create(callLine + 1, 0)
  ))
  
  // Extract indentation (spaces or tabs at the start of the line)
  const indentMatch = currentLineText.match(/^(\s*)/)
  const indent = indentMatch ? indentMatch[1] : ''
  
  return `${indent}// @it-throws\n`
}

// Make the text document manager listen on the connection
// for open, change and close text document events
documents.listen(connection)

// Listen on the connection
connection.listen()

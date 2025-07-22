import {
  DidChangeConfigurationNotification,
  InitializeParams,
  InitializeResult,
  ProposedFeatures,
  TextDocumentSyncKind,
  TextDocuments,
  createConnection,
  HoverParams,
  Hover,
  MarkupKind,
  CodeActionParams,
  CodeAction,
  CodeActionKind,
  TextEdit,
  WorkspaceEdit,
  Range,
  Position
} from 'vscode-languageserver/node'

import { access, constants, readFile } from 'fs/promises'
import { TextDocument } from 'vscode-languageserver-textdocument'
import { InputData, ParseResult, parse_js } from './rust/does_it_throw_wasm'
import path = require('path')
import { inspect } from 'util'

const connection = createConnection(ProposedFeatures.all)

const documents: TextDocuments<TextDocument> = new TextDocuments(TextDocument)
let hasConfigurationCapability = false
let hasWorkspaceFolderCapability = false

// Cache analysis results for hover and code actions
const analysisCache = new Map<string, ParseResult>()

connection.onInitialize((params: InitializeParams) => {
  const capabilities = params.capabilities

  // Does the client support the `workspace/configuration` request?
  // If not, we fall back using global settings.
  hasConfigurationCapability = !!(capabilities.workspace && !!capabilities.workspace.configuration)
  hasWorkspaceFolderCapability = !!(capabilities.workspace && !!capabilities.workspace.workspaceFolders)

  const result: InitializeResult = {
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Incremental,
      hoverProvider: true,
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
  ignoreStatements: ['@it-throws', '@does-it-throw-ignore']
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
    globalSettings = <Settings>(change.settings.doesItThrow || defaultSettings)
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
      section: 'doesItThrow'
    })
    documentSettings.set(resource, result)
  }
  return result
}

// Only keep settings for open documents
documents.onDidClose((e) => {
  documentSettings.delete(e.document.uri)
  analysisCache.delete(e.document.uri)
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

// Add hover support
connection.onHover((params: HoverParams): Hover | null => {
  const document = documents.get(params.textDocument.uri)
  if (!document) {
    return null
  }

  const analysis = analysisCache.get(params.textDocument.uri)
  if (!analysis) {
    return null
  }

  const position = params.position
  const offset = document.offsetAt(position)

  // Check if hovering over a function call that might throw
  const hoverInfo = findFunctionCallAtPosition(analysis, offset, document)
  if (hoverInfo) {
    const { functionName, errorTypes, isDocumented } = hoverInfo
    
    if (!isDocumented && errorTypes.length > 0) {
      const suggestedComment = `/**\n${errorTypes.map((type: string) => ` * @throws {${type}}`).join('\n')}\n */`
      const markdown = [
        `**Function Call**: \`${functionName}()\``,
        '',
        `**May throw**: ${errorTypes.map((type: string) => `\`${type}\``).join(', ')}`,
        '',
        '💡 **Suggestion**: Add throws annotation to document potential errors:',
        '```javascript',
        suggestedComment,
        `function yourFunction() {`,
        `  ${functionName}(); // This call may throw`,
        `}`,
        '```',
        '',
        '*Use Ctrl+. (Cmd+. on Mac) for quick fix*'
      ].join('\n')

      return {
        contents: {
          kind: MarkupKind.Markdown,
          value: markdown
        }
      }
    }
  }

  return null
})

// Add code action support
connection.onCodeAction((params: CodeActionParams): CodeAction[] => {
  const document = documents.get(params.textDocument.uri)
  if (!document) {
    return []
  }

  const analysis = analysisCache.get(params.textDocument.uri)
  if (!analysis) {
    return []
  }

  const actions: CodeAction[] = []
  const range = params.range

  // NEW: Check for partial documentation diagnostics with quick fix data
  for (const diagnostic of analysis.diagnostics) {
    const diagnosticRange = Range.create(
      Position.create(diagnostic.range.start.line, diagnostic.range.start.character),
      Position.create(diagnostic.range.end.line, diagnostic.range.end.character)
    )
    
    // Check if diagnostic range overlaps with the requested range
    if (rangesOverlap(range, diagnosticRange)) {
      
      // 1. Handle partial documentation (add missing @throws to existing JSDoc)
      if (diagnostic.message.includes('JSDoc defines') && diagnostic.data) {
        const data = diagnostic.data as any
        if (data.quickFixType === 'addMissingThrows') {
          const undocumentedTypes = data.undocumentedTypes as string[]
          
          const jsdocPosition = findJSDocCommentPosition(document, diagnosticRange.start)
          if (jsdocPosition) {
            const newThrowsLines = undocumentedTypes.map(errorType => 
              ` * @throws {${errorType}} TODO: Add description`
            ).join('\n')
            
            const edit: WorkspaceEdit = {
              changes: {
                [params.textDocument.uri]: [
                  TextEdit.insert(jsdocPosition, `${newThrowsLines}\n`)
                ]
              }
            }

            actions.push({
              title: `Add missing @throws: ${undocumentedTypes.join(', ')}`,
              kind: CodeActionKind.QuickFix,
              edit: edit,
              isPreferred: true,
              diagnostics: [diagnostic]
            })
          }
        }
      }
      
      // 2. Handle try/catch suggestion (wrap function call)
      else if (diagnostic.data) {
        const data = diagnostic.data as any
        if (data.quickFixType === 'addTryCatch') {
          const functionName = data.functionName as string
          const errorTypes = data.errorTypes as string[]
          
          // Find the line containing the function call
          const lineText = document.getText(Range.create(
            Position.create(diagnosticRange.start.line, 0),
            Position.create(diagnosticRange.start.line + 1, 0)
          ))
          
          // Find the indentation of the current line
          const indentMatch = lineText.match(/^(\s*)/)
          const indent = indentMatch ? indentMatch[1] : ''
          
          // Create try/catch wrapper
          const tryStart = `${indent}try {\n${indent}  `
          const catchBlock = `\n${indent}} catch (error) {\n${indent}  // Handle ${errorTypes.join(', ')} error\n${indent}  console.error('Error in ${functionName}():', error);\n${indent}  // TODO: Add proper error handling\n${indent}}`
          
          const edit: WorkspaceEdit = {
            changes: {
              [params.textDocument.uri]: [
                TextEdit.insert(Position.create(diagnosticRange.start.line, 0), tryStart),
                TextEdit.insert(Position.create(diagnosticRange.end.line + 1, 0), catchBlock)
              ]
            }
          }

          actions.push({
            title: `Wrap ${functionName}() call in try/catch`,
            kind: CodeActionKind.QuickFix,
            edit: edit,
            isPreferred: false, // Less preferred than JSDoc option
            diagnostics: [diagnostic]
          })
        }
        
        // 3. Handle JSDoc @throws suggestion (add to containing function)
        else if (data.quickFixType === 'addJSDocThrows') {
          const functionName = data.functionName as string
          const errorTypes = data.errorTypes as string[]
          
          // Find the containing function
          const containingFunction = findContainingFunction(document, diagnosticRange.start)
          
          if (containingFunction) {
            // Check if function already has JSDoc
            const existingJSDocPosition = findJSDocCommentPosition(document, containingFunction.start)
            
            if (existingJSDocPosition) {
              // Add to existing JSDoc
              const newThrowsLines = errorTypes.map(errorType => 
                ` * @throws {${errorType}} TODO: Add description`
              ).join('\n')
              
              const edit: WorkspaceEdit = {
                changes: {
                  [params.textDocument.uri]: [
                    TextEdit.insert(existingJSDocPosition, `${newThrowsLines}\n`)
                  ]
                }
              }

              actions.push({
                title: `Add @throws to JSDoc: ${errorTypes.join(', ')}`,
                kind: CodeActionKind.QuickFix,
                edit: edit,
                isPreferred: true, // Preferred over try/catch
                diagnostics: [diagnostic]
              })
            } else {
              // Create new JSDoc block
              const jsdocBlock = [
                '/**',
                ...errorTypes.map(errorType => ` * @throws {${errorType}} TODO: Add description`),
                ' */'
              ].join('\n')
              
              const edit: WorkspaceEdit = {
                changes: {
                  [params.textDocument.uri]: [
                    TextEdit.insert(containingFunction.start, `${jsdocBlock}\n`)
                  ]
                }
              }

              actions.push({
                title: `Add JSDoc with @throws: ${errorTypes.join(', ')}`,
                kind: CodeActionKind.QuickFix,
                edit: edit,
                isPreferred: true, // Preferred over try/catch
                diagnostics: [diagnostic]
              })
            }
          }
        }
      }
    }
  }

  return actions
})

function findJSDocCommentPosition(document: TextDocument, functionStart: Position): Position | null {
  const text = document.getText()
  
  // Work backwards from the function to find the JSDoc comment
  let currentLine = functionStart.line - 1
  
  while (currentLine >= 0) {
    const lineText = document.getText(Range.create(
      Position.create(currentLine, 0),
      Position.create(currentLine + 1, 0)
    )).trim()
    
    // Look for the end of a JSDoc comment
    if (lineText.includes('*/')) {
      // Insert before the closing */
      const line = document.getText(Range.create(
        Position.create(currentLine, 0),
        Position.create(currentLine + 1, 0)
      ))
      
      // Find where */ appears in the line
      const closingIndex = line.indexOf('*/')
      if (closingIndex !== -1) {
        return Position.create(currentLine, closingIndex)
      }
    }
    
    // If we hit a line that doesn't look like a comment, stop searching
    if (!lineText.startsWith('*') && !lineText.startsWith('/**') && lineText.length > 0) {
      break
    }
    
    currentLine--
  }
  
  return null
}

// Helper functions for hover and code actions
function findFunctionCallAtPosition(analysis: ParseResult, offset: number, document: TextDocument): { functionName: string, errorTypes: string[], isDocumented: boolean } | null {
  // Find diagnostics that represent function calls at the given position
  for (const diagnostic of analysis.diagnostics) {
    const start = document.offsetAt(Position.create(diagnostic.range.start.line, diagnostic.range.start.character))
    const end = document.offsetAt(Position.create(diagnostic.range.end.line, diagnostic.range.end.character))
    
    if (offset >= start && offset <= end) {
      // Check if this is a function call diagnostic
      if (diagnostic.message.includes('calls') && diagnostic.message.includes('which throws')) {
        // Extract function name and error types from message
        // Updated regex to match: "Function calls XYZ() which throws ABC, DEF - add..."
        const match = diagnostic.message.match(/Function calls (\w+)\(\) which throws (.+?) -/)
        if (match) {
          const functionName = match[1]
          const errorTypesStr = match[2]
          const errorTypes = errorTypesStr.split(', ').map((type: string) => type.trim())
          
          return {
            functionName,
            errorTypes,
            isDocumented: false
          }
        }
      }
    }
  }
  
  return null
}

function findFunctionCallsInRange(analysis: ParseResult, range: Range, document: TextDocument): Array<{ position: Position, functionName: string, errorTypes: string[], isDocumented: boolean }> {
  const calls: Array<{ position: Position, functionName: string, errorTypes: string[], isDocumented: boolean }> = []
  
  for (const diagnostic of analysis.diagnostics) {
    const diagnosticRange = Range.create(
      Position.create(diagnostic.range.start.line, diagnostic.range.start.character),
      Position.create(diagnostic.range.end.line, diagnostic.range.end.character)
    )
    
    // Check if diagnostic range overlaps with the given range
    if (rangesOverlap(range, diagnosticRange)) {
      if (diagnostic.message.includes('calls') && diagnostic.message.includes('which throws')) {
        // Updated regex to match: "Function calls XYZ() which throws ABC, DEF - add..."
        const match = diagnostic.message.match(/Function calls (\w+)\(\) which throws (.+?) -/)
        if (match) {
          const functionName = match[1]
          const errorTypesStr = match[2]
          const errorTypes = errorTypesStr.split(', ').map((type: string) => type.trim())
          
          calls.push({
            position: Position.create(diagnostic.range.start.line, diagnostic.range.start.character),
            functionName,
            errorTypes,
            isDocumented: false
          })
        }
      }
    }
  }
  
  return calls
}

function rangesOverlap(range1: Range, range2: Range): boolean {
  const start1 = range1.start
  const end1 = range1.end
  const start2 = range2.start
  const end2 = range2.end
  
  return !(
    (end1.line < start2.line || (end1.line === start2.line && end1.character < start2.character)) ||
    (end2.line < start1.line || (end2.line === start1.line && end2.character < start1.character))
  )
}

function findContainingFunction(document: TextDocument, position: Position): { start: Position, name: string } | null {
  const text = document.getText()
  const offset = document.offsetAt(position)
  
  // Simple regex to find function declarations before the position
  const functionRegex = /(?:function\s+(\w+)|(\w+)\s*(?::\s*\w+)?\s*=>|(\w+)\s*\([^)]*\)\s*{)/g
  
  let match
  let lastFunction: { start: Position, name: string } | null = null
  
  while ((match = functionRegex.exec(text)) !== null) {
    if (match.index > offset) {
      break
    }
    
    const functionName = match[1] || match[2] || match[3] || 'anonymous'
    const functionStart = document.positionAt(match.index)
    
    lastFunction = {
      start: functionStart,
      name: functionName
    }
  }
  
  return lastFunction
}

function getCommentInsertPosition(document: TextDocument, func: { start: Position, name: string }): Position {
  const line = func.start.line
  const lineText = document.getText(Range.create(Position.create(line, 0), Position.create(line + 1, 0)))
  
  // Find the end of the function signature (before the opening brace)
  const braceIndex = lineText.indexOf('{')
  if (braceIndex !== -1) {
    return Position.create(line, braceIndex)
  }
  
  // Fallback: end of the line
  return Position.create(line, lineText.trim().length)
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
      uri: textDocument.uri,
      file_content: textDocument.getText(),
      ids_to_check: [],
      typescript_settings: {
        decorators: true
      },
      function_throw_severity: settings?.functionThrowSeverity ?? defaultSettings.functionThrowSeverity,
      throw_statement_severity: settings?.throwStatementSeverity ?? defaultSettings.throwStatementSeverity,
      call_to_imported_throw_severity:
        settings?.callToImportedThrowSeverity ?? defaultSettings.callToImportedThrowSeverity,
      call_to_throw_severity: settings?.callToThrowSeverity ?? defaultSettings.callToThrowSeverity,
      include_try_statement_throws: settings?.includeTryStatementThrows ?? defaultSettings.includeTryStatementThrows,
      ignore_statements: settings?.ignoreStatements ?? defaultSettings.ignoreStatements
    } satisfies InputData
    const analysis = parse_js(opts) as ParseResult

    // Cache the analysis for hover and code actions
    analysisCache.set(textDocument.uri, analysis)

    if (analysis.relative_imports.length > 0) {
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
          uri: file.fileUri,
          file_content: file.fileContent,
          ids_to_check: [],
          typescript_settings: {
            decorators: true
          }
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
            const newDiagnostics = analysis.imported_identifiers_diagnostics.get(throw_id)
            if (newDiagnostics?.diagnostics?.length) {
              analysis.diagnostics.push(...newDiagnostics.diagnostics)
            }
          }
        }
      }
    }
    connection.sendDiagnostics({
      uri: textDocument.uri,
      diagnostics: analysis.diagnostics
    })
  } catch (e) {
    console.log(e)
    connection.console.error(`Error parsing file ${textDocument.uri}`)
    connection.console.error(`settings are: ${JSON.stringify(settings)}`)
    connection.console.error(`Error: ${e instanceof Error ? e.message : JSON.stringify(e)} error`)
    connection.sendDiagnostics({ uri: textDocument.uri, diagnostics: [] })
    // Clear cache on error
    analysisCache.delete(textDocument.uri)
  }
}

connection.onDidChangeWatchedFiles((_change) => {
  // Monitored files have change in VSCode
  connection.console.log(`We received an file change event ${_change}, not implemented yet`)
})

// Make the text document manager listen on the connection
// for open, change and close text document events
documents.listen(connection)

// Listen on the connection
connection.listen()

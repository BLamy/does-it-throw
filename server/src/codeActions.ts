import { TextDocument } from 'vscode-languageserver-textdocument'
import { Range, Position, CodeActionKind } from 'vscode-languageserver/node'

export interface DiagnosticLike {
  message: string
  range: Range
  source?: string
}

export interface QuickFixEditLike {
  range: Range
  newText: string
}

export interface QuickFixLike {
  title: string
  kind: string
  edits: QuickFixEditLike[]
}

// Helper function to find the position to insert code at the start of a catch block
function findInsertPositionStart(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  const startLine = diagnosticRange.start.line
  for (let line = startLine; line < Math.min(startLine + 10, textDocument.lineCount); line++) {
    const lineText = textDocument.getText(Range.create(
      Position.create(line, 0),
      Position.create(line + 1, 0)
    ))
    if (lineText.match(/catch\s*\([^)]+\)\s*\{/)) {
      return Position.create(line + 1, 0)
    }
  }
  return null
}

// Helper function to find the position to insert code at the end of a catch block (before closing brace)
function findInsertPositionEnd(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  const startLine = diagnosticRange.start.line
  let braceCount = 0
  let foundCatchStart = false
  for (let line = startLine; line < Math.min(startLine + 20, textDocument.lineCount); line++) {
    const lineText = textDocument.getText(Range.create(
      Position.create(line, 0),
      Position.create(line + 1, 0)
    ))
    if (!foundCatchStart && lineText.match(/catch\s*\([^)]+\)\s*\{/)) {
      foundCatchStart = true
      braceCount = 1
      continue
    }
    if (foundCatchStart) {
      for (const char of lineText) {
        if (char === '{') braceCount++
        if (char === '}') braceCount--
        if (braceCount === 0) {
          return Position.create(line, 0)
        }
      }
    }
  }
  return null
}

// Helper function to find the position to insert JSDoc before a function
function findJSDocInsertPosition(textDocument: TextDocument, diagnosticRange: Range): Position | null {
  const startLine = diagnosticRange.start.line
  let insertLine = startLine
  for (let line = startLine - 1; line >= 0; line--) {
    const lineText = textDocument.getText(Range.create(
      Position.create(line, 0),
      Position.create(line + 1, 0)
    )).trim()
    if (lineText && !lineText.startsWith('//') && !lineText.startsWith('/*') && !lineText.startsWith('*')) {
      insertLine = line + 1
      break
    }
    if (line === 0) {
      insertLine = 0
      break
    }
  }
  return Position.create(insertLine, 0)
}

// Helper function to find the position to insert @it-throws comment before a function call
function findCommentInsertPosition(_textDocument: TextDocument, diagnosticRange: Range): Position | null {
  const callLine = diagnosticRange.start.line
  return Position.create(callLine, 0)
}

// Helper function to get properly indented @it-throws comment
function getIndentedComment(textDocument: TextDocument, diagnosticRange: Range): string {
  const callLine = diagnosticRange.start.line
  const currentLineText = textDocument.getText(Range.create(
    Position.create(callLine, 0),
    Position.create(callLine + 1, 0)
  ))
  const indentMatch = currentLineText.match(/^(\s*)/)
  const indent = indentMatch ? indentMatch[1] : ''
  return `${indent}// @it-throws\n`
}

export function computeQuickFixesForDiagnostics(
  fileContent: string,
  diagnostics: DiagnosticLike[]
): QuickFixLike[] {
  const textDocument = TextDocument.create('file://test.ts', 'typescript', 1, fileContent)
  const codeActions: QuickFixLike[] = []

  for (const diagnostic of diagnostics) {
    if (diagnostic.source === 'Does it Throw?' && diagnostic.message.includes('Exhaustive catch is missing handlers for:')) {
      const match = diagnostic.message.match(/missing handlers for: ([^.]+)/)
      if (match) {
        const missingTypes = match[1].split(', ').map(type => type.trim())
        const insertPositionStart = findInsertPositionStart(textDocument, diagnostic.range)
        const insertPositionEnd = findInsertPositionEnd(textDocument, diagnostic.range)
        if (insertPositionStart) {
          const handlersText = missingTypes.map(errorType =>
            `    if (e instanceof ${errorType}) {\n      // Handle ${errorType}\n      console.error('${errorType}:', e.message);\n      return null;\n    }`
          ).join(' else ') + '\n'
          codeActions.push({
            title: `Add instanceof handlers for ${missingTypes.join(', ')}`,
            kind: CodeActionKind.QuickFix,
            edits: [{ range: Range.create(insertPositionStart, insertPositionStart), newText: handlersText }]
          })
        }
        if (insertPositionEnd) {
          codeActions.push({
            title: "Add 'throw e' as escape hatch",
            kind: CodeActionKind.QuickFix,
            edits: [{ range: Range.create(insertPositionEnd, insertPositionEnd), newText: '    // Escape hatch for unhandled errors\n    throw e;\n' }]
          })
        }
      }
    } else if (diagnostic.source === 'Does it Throw?' &&
               (diagnostic.message.match(/^Function .+ may throw(?:: .+)?$/) || diagnostic.message.startsWith('Anonymous function may throw'))) {
      const anon = diagnostic.message.startsWith('Anonymous function may throw')
      const extracted = diagnostic.message.match(/^Function (.+) may throw(?:: (.+))?$/)
      const typesPart = extracted && extracted[2] ? extracted[2] : ''
      const errorTypes = typesPart.length > 0 ? typesPart.split(', ').map((t: string) => t.trim()) : ['Error']
      const insertPosition = findJSDocInsertPosition(textDocument, diagnostic.range)
      if (insertPosition) {
        const jsdocLines = [
          '/**',
          ...errorTypes.map((errorType: string) => ` * @throws {${errorType}}`),
          ' */'
        ]
        const jsdocText = jsdocLines.join('\n') + '\n'
        codeActions.push({
          title: anon ? 'Annotate anonymous function with @throws' : `Add JSDoc @throws for ${errorTypes.join(', ')}`,
          kind: CodeActionKind.QuickFix,
          edits: [{ range: Range.create(insertPosition, insertPosition), newText: jsdocText }]
        })
      }
      if (anon) {
        const range = diagnostic.range
        const callbackText = textDocument.getText(range)
        const replacement = callbackText
          .replace(/\(\s*([^)]*)\s*\)\s*=>\s*\{/, '/**\n * @throws {Error}\n */\nfunction callbackThrows($1) {')
          .replace(/\)\s*=>\s*([^\{][^;]*);?$/, ') { return $1; }')
        codeActions.push({
          title: 'Convert to named function with @throws',
          kind: CodeActionKind.QuickFix,
          edits: [{ range, newText: replacement }]
        })
      }
    } else if (diagnostic.source === 'Does it Throw?' && diagnostic.message === 'Function call may throw: {Error}.') {
      const insertPosition = findCommentInsertPosition(textDocument, diagnostic.range)
      if (insertPosition) {
        codeActions.push({
          title: 'Add @it-throws comment to suppress warning',
          kind: CodeActionKind.QuickFix,
          edits: [{ range: Range.create(insertPosition, insertPosition), newText: getIndentedComment(textDocument, diagnostic.range) }]
        })
      }
    }
  }
  return codeActions
}

export type { Range, Position }



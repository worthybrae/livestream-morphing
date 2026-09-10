import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
const root = fileURLToPath(new URL('.', import.meta.url))
if (!process.argv[2]) throw new Error('Pass the portfolio checkout directory.')
const portfolio = resolve(process.argv[2])
const binary = readFileSync(resolve(root, 'target/wasm32-unknown-unknown/release/ink_wasm.wasm'))
const exports = new WebAssembly.Instance(new WebAssembly.Module(binary)).exports
if (!exports.ink_init(3, 3)) throw new Error('Renderer initialization failed.')
const definitions = JSON.parse(new TextDecoder().decode(new Uint8Array(exports.memory.buffer, exports.ink_definitions(), exports.ink_definitions_len())))
mkdirSync(resolve(portfolio, 'public/media/ink-studio'), { recursive: true })
writeFileSync(resolve(portfolio, 'public/media/ink-studio/ink.wasm'), binary)
writeFileSync(resolve(portfolio, 'src/components/LiveStream/inkParams.json'), JSON.stringify(definitions, null, 2) + '\n')
console.log(`Published ${binary.length} bytes and ${definitions.length} parameter definitions.`)

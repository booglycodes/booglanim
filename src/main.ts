// main booglanim editor
import * as monaco from 'monaco-editor'
import * as ts from "typescript";
import { World } from './world';
import { Run } from './anim';
import { invoke } from '@tauri-apps/api';

// sources for all the things we can import in the booglanim editor
import anim_src from './anim.ts?raw'
import character_src from './character.ts?raw'
import obj_src from './obj.ts?raw'
import point_src from './point.ts?raw'
import resources_src from './resources.ts?raw'
import world_src from './world.ts?raw'

// default text we put in the editor
import editor_default from './editor_default.template?raw'
import { save } from '@tauri-apps/api/dialog';
import { writeTextFile } from '@tauri-apps/api/fs';

let world: World
let frame: number = 0
let runningEditorCode: boolean = false

// runs a single tick of the world
function worldTick() {
    let runningTotal = 0
    for (const task of world.tasks) {
        let relative_frame = frame - runningTotal
        if (Array.isArray(task)) {
            let parallel_tasks = (task as Run[])
            let last_frame = Math.max(...parallel_tasks.map((run) => run.lastFrame))
            if (frame < runningTotal) {
                break;
            }
            parallel_tasks.forEach((task) => {
                task.run(relative_frame)
            })
            if (frame >= runningTotal) {
                runningTotal += last_frame
            }
        } else if ('run' in task && 'lastFrame' in task) {
            let t = (task as Run)
            t.run(relative_frame)
            runningTotal += t.lastFrame
        } else {
            (task as ((frame: number) => void))(relative_frame)
        }
    }
    world.things.sort((a, b) => a.layer - b.layer)
    frame++
    return runningTotal
}

// sources we can import and their names
let booglyStudioImports = [
    ['anim', anim_src],
    ['character', character_src],
    ['obj', obj_src],
    ['point', point_src],
    ['resources', resources_src],
    ['world', world_src]
]

// create the monaco editor and add the libraries from booglyStudioImports to it
let editor = monaco.editor.create(document.getElementById('container')!, {
    value: editor_default,
    language: 'typescript',
    automaticLayout: true,
    theme: 'vs-dark',
})
for (const booglyStudioImport of booglyStudioImports) {
    let name = booglyStudioImport[0]
    let src = booglyStudioImport[1]
    monaco.languages.typescript.typescriptDefaults.addExtraLib(
        `declare module '@booglanim/${name}' { ${src} }`
    );
}

// Runs the editor code. Does this by eval'ing it. Note that the editor code is an async closure 
// that is immediately called, we can't await it. Instead, we can poll the `runningEditorCode` flag
// and await a timeout promise in a loop. as an example:
// ```
//     while (runningEditorCode) {
//         await new Promise(x => setTimeout(x, 100))
//     }
// ```
// this will essentially stop the execution here and poll every 100ms to check if the editor is done 
// running the async code, without completely blocking the thread.
function runEditorCode() {
    eval(typescriptToEvalableJavascript(editor.getValue()))
    return true
}

// Do a bunch of junky modifications to the typescript so that we can eval it in this context. 
// The only really hacky and terrible part of this codebase.
function typescriptToEvalableJavascript(program: string) {
    // split program into imports and code, works under the assumption that we only have one async function named getWorld.
    let [imports, code] = program.split('async function')

    // wrap all the code in a closure that gets immediately evaluated (see parens at the end).
    // before the closure, put all the imports and set runningEditorCode = true
    // also, we await getWorld() because we assume that the editor is providing it. Once that function has completed, we can set 
    // runningEditorCode = false; to unblock whoever is waiting for the editor code to finish. 
    let wrapper = imports + 'runningEditorCode = true;\n(async () => {\nasync function ' + code + '\nworld = await getWorld(); runningEditorCode = false; })()'
    let transpile = ts.transpile(wrapper)

    // generate imports by looking for the variable names in the require statements
    let match = null
    let fixedImports = ''
    let variableNameRegex = /var\s+(.*?)\s+=\s+require/g
    while ((match = variableNameRegex.exec(transpile)) !== null) {
        fixedImports += 'let ' + match[1] + ' = await import("./' + match[1].substring(0, match[1].lastIndexOf('_')) + '.ts");'
    }

    return transpile
        // just get rid of this line of code, no idea what it does but it throws an error when we eval it
        .split('Object.defineProperty(exports, "__esModule", { value: true });').join('')

        // add in our imports here and convert function to async so that we can await in it (import function is async, we need to await it).
        .split('(function () { return __awaiter(void 0, void 0, void 0, function () {')
        .join('(async function () { ' + fixedImports + ' return __awaiter(void 0, void 0, void 0, function () {')

        // get rid of the original imports by replacing the `require` function calls with null
        .replace(new RegExp(/require\(.*?\)/, 'g'), 'null')
}

let finishedBuild = false
let lastFrame = Infinity
document.getElementById('build')?.addEventListener('click', async () => {
    frame = 0
    finishedBuild = false
    let frameCounter: HTMLElement = document.getElementById('status')!
    frameCounter.textContent = "building - running editor code..."
    if (!runEditorCode()) {
        alert("Your code doesn't compile, can't make the video")
        return
    }

    while (runningEditorCode) {
        await new Promise(x => setTimeout(x, 100))
    }

    frameCounter.textContent = "building - updating media resources..."
    await invoke('update_media_resources', { res: world.res.serialize(), fps : world.fps })
    frameCounter.textContent = "building frames..."
    await new Promise(x => setTimeout(x, 0))
    let frames = []
    lastFrame = Infinity
    while (frame < lastFrame) {
        frames.push(structuredClone(world.things))
        lastFrame = worldTick()
    }
    frameCounter.textContent = "sending frames..."
    await invoke('add_frames', {frames : frames})
    frameCounter.textContent = "build complete!"
    finishedBuild = true
})

for (const button of ['play', 'pause', 'stop']) {
    document.getElementById(button)?.addEventListener('click', async () => {
        await invoke(button)
    })
}

document.getElementById('save')?.addEventListener('click', async () => {
    let filePath = await save()
    writeTextFile(filePath!, editor.getValue())
})

document.getElementById('export')?.addEventListener('click', async () => {
    if (!finishedBuild) {
        alert("you need to finish building before you can 'export'")
        return
    }
    
    let filePath = await save()
    if (filePath?.endsWith('.mp4')) {
        await invoke('export', { path : filePath! })
    } else {
        alert('invalid filepath, must end with .mp4')
    }
})

import { listen } from '@tauri-apps/api/event'

listen('encoded-frame', (event) => {
        let frameCounter: HTMLElement = document.getElementById('status')!
    let frame = event.payload as number + 1;
    if (frame === lastFrame) {
        frameCounter.textContent = "finished rendering video!"
    } else {
        frameCounter.textContent = (event.payload as number + 1).toString() + " frame(s) out of " + lastFrame + " completed"
    }
})
import { Point, dist } from '../point.js'
import { drawCharacter, drawLimb, drawLimbDebug, drawMode, JointSelection, closestJoint, loadCharacterFrontend } from "./character_interface_utils.js"
import { listen } from '@tauri-apps/api/event'
import { writeTextFile } from '@tauri-apps/api/fs'
import { save } from '@tauri-apps/api/dialog'
import { Character, Limb, canvasToCharacterRelative, characterRelativeToCanvas } from '../character.js'

const FPS = 12

type RenderMode = 'debug' | 'normal'

function mod(n : number, m : number) {
    return ((n % m) + m) % m
}

let frameData : Limb[][]
let frame = 0
let numFrames = 1
let renderMode : RenderMode = 'debug'
let playing = false
window.addEventListener("keyup", async (e) => {
    if (e.key === 'n') {
        frameData.push(frameData[frameData.length - 1])
        frame = numFrames
        numFrames++
    }
    if (e.key === 'a') {
        frame = mod(frame - 1, numFrames)
    }
    if (e.key === 'd') {
        frame = mod(frame + 1, numFrames)
    }
    if (e.key === ' ') {
        playing = !playing
    }
    if (e.key === 't') {
        renderMode = renderMode === 'debug' ? 'normal' : 'debug'
    }
    if (e.key === '=') {
        character.scale += 10
    }
    if (e.key === '-' && character.scale > 50) {
        character.scale -= 10
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        let filePath = await save({
            filters: [{
                name: 'JSON',
                extensions: ['json']
            }]
        })
        writeTextFile(filePath!, JSON.stringify(frameData))
    }
    draw()
})

let character : Character
listen('tauri://file-drop', async event => {
    character = await loadCharacterFrontend((event.payload as string[])[0])
    frameData = [character.limbs]
    draw()
})

let ctx: CanvasRenderingContext2D
let canvas: HTMLCanvasElement
window.addEventListener("DOMContentLoaded", () => {
    canvas = document.querySelector("#characterCanvas")!
    ctx = canvas.getContext("2d")!
})

function pt(x : number, y : number): Point {
    return {x : x, y : y}
}

function drawPolygon(ctx: CanvasRenderingContext2D, points : Point[], color : string) {
    ctx.beginPath()
    ctx.moveTo(points[0].x, points[0].y)
    points.slice(1).forEach((point) => ctx.lineTo(point.x, point.y))
    ctx.closePath()
    ctx.fillStyle = color
    ctx.fill()
}

function draw() {
    canvas.width = window.innerWidth
    canvas.height = window.innerHeight
    ctx.clearRect(0, 0, canvas.width, canvas.height)
    character.limbs = frameData[frame]
    if (renderMode === 'debug') {
        drawCharacter(ctx, character, drawLimbDebug)
    } else {
        drawCharacter(ctx, character, drawLimb)
    }
    drawMode(ctx, renderMode + ' ' + (frame + 1) + '/' + numFrames)
    if (playing) {
        drawPolygon(ctx, [pt(200, 10), pt(200, 40), pt(220, 25)], '#00AA00')
        frame = mod(frame + 1, numFrames)
        setTimeout(() => {
            requestAnimationFrame(draw)
        }, 1000 / FPS);
    } else {
        drawPolygon(ctx, [pt(200, 10), pt(200, 40), pt(230, 40), pt(230, 10)], '#AA0000')
    }
}

function posn(e: MouseEvent): Point {
    return {
        x: e.clientX - canvas.getBoundingClientRect().x,
        y: e.clientY - canvas.getBoundingClientRect().y,
    }
}

let drag = false
let selectedJoint: JointSelection | null = null
window.addEventListener("mousedown", (e) => {
    if (playing || character === undefined) {
        return
    }
    selectedJoint = null
    drag = true

    let maybeSelection = closestJoint(character, posn(e))
    if (maybeSelection === null) {
        return
    }
    let point = characterRelativeToCanvas(character.limbs[maybeSelection![0]].points[maybeSelection![1]], character)
    if (dist(point, posn(e)) < 50) {
        selectedJoint = maybeSelection
    }
})

window.addEventListener('mousemove', (e) => {
    if (playing || character === undefined || !drag) {
        return
    }
    let currPosn = posn(e)
    if (selectedJoint !== null) {
        character.limbs[selectedJoint![0]].points[selectedJoint![1]] = canvasToCharacterRelative(currPosn, character)
        frameData[frame] = structuredClone(character.limbs)
    } else {
        character.pos = currPosn
    }
    draw()
})

window.addEventListener('mouseup', () => drag = false)

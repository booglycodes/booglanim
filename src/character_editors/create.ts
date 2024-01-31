// character creator interface
import { Point, dist } from "../point.js"
import { drawMode, drawCharacter, drawLimbDebug, JointSelection, closestJoint } from "./character_interface_utils.js"
import { save } from '@tauri-apps/api/dialog'
import { listen } from '@tauri-apps/api/event'
import { writeTextFile } from '@tauri-apps/api/fs'
import { convertFileSrc, invoke } from '@tauri-apps/api/tauri'
import { Character, canvasToCharacterRelative, characterRelativeToCanvas } from "../character.js"

let character: Character
let ctx: CanvasRenderingContext2D
let canvas: HTMLCanvasElement
window.addEventListener("DOMContentLoaded", () => {
    canvas = document.querySelector("#characterCanvas")!
    ctx = canvas.getContext("2d")!
    draw()
});

window.addEventListener("resize", () => {
    draw()
})

interface Line {
    start: Point,
    end: Point,
    color: string
}

function posn(e: MouseEvent): Point {
    return {
        x: e.clientX - canvas.getBoundingClientRect().x,
        y: e.clientY - canvas.getBoundingClientRect().y,
    }
}

type Interaction = "move" | "limb" | "delete"

let selectedJoint: JointSelection | null = null
let dragLine: Line | null = null
let mouseStart: Point
let mouseEnd: Point
let drag = false
// we use this, which we only change on mousedown
// so you can't change the mode in the middle of doing an action
let freezeMode: Interaction = "move"
window.addEventListener("mousedown", (e) => {
    mouseStart = posn(e)
    freezeMode = mode
    drag = true
    selectedJoint = null

    let maybeSelection = closestJoint(character, posn(e))
    if (maybeSelection === null) {
        return
    }
    let point = characterRelativeToCanvas(character.limbs[maybeSelection![0]].points[maybeSelection![1]], character)
    if (dist(point, mouseStart) < 50) {
        selectedJoint = maybeSelection
    }
})

window.addEventListener('mousemove', (e) => {
    if (!drag) {
        return
    }
    let currPosn = posn(e)
    if (freezeMode === 'limb') {
        dragLine = {
            start: mouseStart,
            end: currPosn,
            color: '#CC0099'
        }
    }
    if (freezeMode === 'move' && selectedJoint !== null) {
        character.limbs[selectedJoint![0]].points[selectedJoint![1]] = canvasToCharacterRelative(currPosn, character)
    } else if (freezeMode === 'move') {
        character.pos = currPosn
    }
    draw()
})

window.addEventListener("mouseup", (e) => {
    drag = false
    mouseEnd = posn(e)
    dragLine = null
    if (freezeMode === 'limb') {
        let pt = canvasToCharacterRelative(mouseEnd, character)
        if (selectedJoint !== null) {
            character.limbs[selectedJoint![0]].points.push(pt)
        } else {
            character.limbs.push({ points: [pt], color : {r : 255, g: 233, b: 209}, thickness : 10 })
        }
    }
    if (freezeMode === 'delete') {
        if (selectedJoint !== null) {
            character.limbs.splice(selectedJoint![0], 1)
        }
    }
    draw()
})

let mode: Interaction = "move"
window.addEventListener("keyup", async (e) => {
    if (e.key === "m") {
        mode = "move"
    }
    if (e.key === "l") {
        mode = "limb"
    }
    if (e.key === 'd') {
        mode = "delete"
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
        console.log(imgPath)
        let base64Img = await invoke('to_base64_png', { path : imgPath })
        character.limbs.forEach(limb => limb.thickness = 0.1)
        writeTextFile(filePath!, JSON.stringify({
            img: base64Img,
            limbs: character.limbs,
            scale: 1
        }))
        character.limbs.forEach(limb => limb.thickness = 10)
    }
    draw()
})

let imgPath : string
listen('tauri://file-drop', event => {
    console.log('drop', event)
    let img = new Image
    imgPath = (event.payload as string[])[0]
    console.log(imgPath)
    img.src = convertFileSrc(imgPath)
    character = {
        img: img,
        pos: { x: canvas.width * 0.5, y: canvas.height * 0.5 },
        scale: 300,
        limbs: [],
        layer: 0,
        visible : true,
        limbsInFront : true
    }
    draw()
})

function draw() {
    if (character === undefined) {
        return
    }
    canvas.width = window.innerWidth
    canvas.height = window.innerHeight
    ctx.clearRect(0, 0, canvas.width, canvas.height)
    drawMode(ctx, mode)

    if (dragLine !== null) {
        ctx.strokeStyle = dragLine.color
        ctx.beginPath()
        ctx.moveTo(dragLine.start.x, dragLine.start.y)
        ctx.lineTo(dragLine.end.x, dragLine.end.y)
        ctx.stroke()
    }

    drawCharacter(ctx, character, drawLimbDebug)
}


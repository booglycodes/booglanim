// utility functions for the character interfaces (drawing it in the js canvas, )
import { BaseDirectory, readTextFile } from "@tauri-apps/api/fs";
import { Point, add, dist, scale } from "../point";
import { Character, Limb, characterRelativeToCanvas } from "../character";

function drawCircle(ctx: CanvasRenderingContext2D, x : number, y : number, radius : number, fill : string) {
    ctx.beginPath()
    ctx.arc(x, y, radius, 0, 2 * Math.PI, false)
    ctx.fillStyle = fill
    ctx.fill()
}

export function drawLimbDebug(ctx : CanvasRenderingContext2D, limb : Limb, character : Character) {
    let joints = limb.points.map((limb) => characterRelativeToCanvas(limb, character))
    ctx.strokeStyle = '#00AA00'
    ctx.lineWidth = 3
    ctx.beginPath()
    ctx.moveTo(joints[0].x, joints[0].y)
    joints.slice(1).forEach((joint) => ctx.lineTo(joint.x, joint.y))
    ctx.stroke()
    joints.forEach((joint) => drawCircle(ctx, joint.x, joint.y, 10, '#003399'))
}

export function drawLimb(ctx : CanvasRenderingContext2D, limb : Limb, character : Character) {
    let joints = limb.points.map((limb) => characterRelativeToCanvas(limb, character))
    ctx.strokeStyle = '#ffe9d1'
    ctx.lineWidth = character.scale * 0.2
    ctx.lineCap = 'round'
    ctx.beginPath()
    ctx.moveTo(joints[0].x, joints[0].y)
    let ctrlpt0 = add(joints[0], scale(0.5, add(joints[1], scale(-1, joints[0]))))
    let ctrlpt1 = add(joints[1], scale(0.5, add(joints[2], scale(-1, joints[1]))))
    ctx.bezierCurveTo(ctrlpt0.x, ctrlpt0.y, ctrlpt1.x, ctrlpt1.y, joints[2].x, joints[2].y)
    ctx.stroke()
}

export function drawCharacter(ctx : CanvasRenderingContext2D, character : Character, drawLimb : (ctx : CanvasRenderingContext2D, limb: Limb, c : Character) => void) {
    let w = character.scale
    let img = character.img as HTMLImageElement;
    let h = img.height / img.width * character.scale 
    ctx.drawImage(img, character.pos.x - w * 0.5, character.pos.y - h * 0.5, w, h)
    character.limbs.forEach((limb : Limb) => drawLimb(ctx, limb, character))
}

export function drawMode(ctx : CanvasRenderingContext2D, mode : string) {
    ctx.font = 'bold 16px verdana sans-serif'
    ctx.fillStyle = '#AA0000'
    ctx.fillText(mode, 20, 20)
}

export type JointSelection = [number, number]

export function closestJoint(character : Character, point : Point): JointSelection | null {
    let minDist = Infinity
    let closestJoint : JointSelection | null = null
    for (let i = 0; i < character.limbs.length; i++) {
        let limb = character.limbs[i]
        for (let j = 0; j < limb.points.length; j++) {
            let joint = characterRelativeToCanvas(limb.points[j], character)
            if (dist(joint, point) < minDist) {
                closestJoint = [i, j]
                minDist = dist(joint, point)
            }
        }
    }
    return closestJoint
}

export async function loadCharacterWithImageHandler(path : string, handleImg : (characterData : any) => HTMLImageElement | number) : Promise<Character> {
    let characterData = JSON.parse(await readTextFile(path, { dir : BaseDirectory.Home }))
    return {
        limbs: characterData.limbs,
        pos: { x: 0, y: 0 },
        scale : characterData.scale,
        img : handleImg(characterData),
        layer : 0,
        visible : true,
        limbsInFront : false
    }
}

export async function loadCharacterFrontend(path : string) : Promise<Character> {
    return await loadCharacterWithImageHandler(path, (characterData : any) => {
        let img = new Image
        img.src = "data:image/png;base64," + characterData.img
        return img
    })
}

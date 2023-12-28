import { Point, pt } from "./point"
import { Resources } from "./resources"

export interface Obj {
    scale: number,
    img: number 
    pos: Point,
    layer: number,
    subrect: {
        topLeft: Point,
        wh: Point
    }
    visible: boolean
}

export async function loadObj(path : string, res : Resources) : Promise<Obj> {
    return {
        scale: 100,
        img: res.add(path),
        pos: pt(0, 0),
        layer: 0,
        subrect: {
            topLeft: pt(0, 0),
            wh: pt(1, 1)
        },
        visible: true
    }
}

export async function loadObjAnim(path : string, frames : number, extension : string, res: Resources) : Promise<number[]> {
    let images = []
    for (let i = 0; i < frames; i++) {
        images.push(res.add(path + '/' + i + '.' + extension))
    }
    return images
}
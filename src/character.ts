import { BaseDirectory, readTextFile } from "@tauri-apps/api/fs"
import { loadCharacterWithImageHandler } from "./character_editors/character_interface_utils"
import { Point, add, scale } from "./point"
import { Resources } from "./resources"

// a boogly character 
export interface Character {
    // img either contains the HTMLImageElement or a 'resource id' that maps to something in the Resources map.
    img : HTMLImageElement | number,
    pos : Point,
    scale : number,
    limbs : Limb[],
    layer : number,
    visible : boolean,
    limbsInFront : boolean
}

// a limb of a character
export interface Limb {
    points : Point[],
    color : {r : number, g : number, b : number},
    thickness : number
}

export function characterRelativeToCanvas(p : Point, character : Character): Point {
    return add(character.pos, scale(character.scale, p))
}

export function canvasToCharacterRelative(p : Point, character : Character) : Point {
    return scale(1/character.scale, add(p, scale(-1, character.pos)))
}

export async function loadCharacter(path : string, res : Resources) : Promise<Character> {
    return await loadCharacterWithImageHandler(path, (_ : any) => res.add(path))
}

export async function loadCharacterAnim(path : string) : Promise<Limb[][]> {
    return JSON.parse(await readTextFile(path, { dir : BaseDirectory.Home }))
}

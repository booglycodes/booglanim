
import { Point, add, pt, scale } from "./point"
import { Obj } from "./obj"
import { Character, Limb, canvasToCharacterRelative, characterRelativeToCanvas } from "./character"

export interface Run {
    run: (frame: number) => void,
    lastFrame: number
}

// do some action from the start frame till the end frame
export function from(startFrame: number, endFrame: number, callback: (frame: number, duration : number) => void): Run {
    return {
        run: (frame: number) => {
            if (frame >= startFrame && frame < endFrame) {
                callback(frame - startFrame, endFrame - startFrame)
            }
        }, lastFrame: endFrame
    }
}

// do some action at a frame
export function at(frame: number, callback: () => void): Run {
    return {
        run: (f: number) => {
            if (f === frame) {
                callback()
            }
        }, lastFrame: frame
    }
}

// run an animation for a character or an object
export function animate(thing: Character | Obj, animation: Limb[][] | number[], reverse?: boolean, bounce?: boolean) {
    return (frame: number, _ : number) => {
        if (animation.length === 0) {
            return
        }
        let animationFrame: number
        if (bounce === undefined || !bounce) {
            animationFrame = frame % animation.length
        } else {
            let l = animation.length - 1
            animationFrame = l - Math.abs(frame % (l * 2) - l)
        }

        if (reverse) {
            animationFrame = animation.length - animationFrame - 1
        }

        if (Array.isArray(animation[0])) {
            (thing as Character).limbs = structuredClone(animation[animationFrame] as Limb[])
        } else {
            thing.img = animation[animationFrame] as number
        }
    }
}

// move a thing to a point 
export function move(thing: Character | Obj, to : Point) {
    let speed : Point
    return (frame: number, endFrame : number) => {
        if (frame == 0) {
            speed = scale(1/endFrame, add(to, scale(-1, thing.pos)))
        }
        thing.pos = add(speed, thing.pos)
    }
}

// resize a thing to a specific size
export function resize(thing: Character | Obj, size: number) {
    let delta : number
    return (frame: number, endFrame : number) => {
        if (frame == 0) {
            delta = (size - thing.scale) / endFrame
        }
        thing.scale += delta
    }
}

// lock some object or character to another object or character
export function lock(thingToLink: Character | Obj, thingToLinkTo: Character | Obj, offset: Point) {
    let relativeOffset: Point
    let relativeScale: number
    return (frame: number) => {
        if (frame === 0) {
            relativeOffset = scale(1 / thingToLinkTo.scale, offset)
            relativeScale = thingToLink.scale / thingToLinkTo.scale
        }
        console.log(frame, relativeOffset, relativeScale, thingToLink, thingToLinkTo)
        thingToLink.pos = add(thingToLinkTo.pos, scale(thingToLinkTo.scale, relativeOffset))
        thingToLink.scale = relativeScale * thingToLinkTo.scale
    }
}

// lock some object or character to a character's limb
export function lockToLimb(thingToLink: Character | Obj, characterToLinkTo: Character, limb: number, joint: number, offset: Point) {
    let relativeOffset: Point
    let relativeScale: number
    return (frame: number) => {
        if (frame === 0) {
            relativeOffset = scale(1 / characterToLinkTo.scale, offset)
            relativeScale = thingToLink.scale / characterToLinkTo.scale
        }
        thingToLink.pos = add(
            characterRelativeToCanvas(characterToLinkTo.limbs[limb].points[joint], characterToLinkTo),
            scale(characterToLinkTo.scale, relativeOffset)
        )
        thingToLink.scale = relativeScale * characterToLinkTo.scale
    }
}

// move a limb of a character to a 
export function moveLimbTo(character: Character, limb: number, joint: number, destination: Obj | Character | Point, offset?: Point) {
    let start: Point
    let diff: Point
    let finish: Point
    return (frame: number, endFrame : number) => {
        if (frame === 0) {
            if (offset === undefined) {
                offset = pt(0, 0)
            }
            let pos : Point
            if ('pos' in destination) {
                pos = destination.pos
            } else {
                pos = destination
            }
            finish = add(pos, offset)
            start = characterRelativeToCanvas(character.limbs[limb].points[joint], character)
            diff = add(finish, scale(-1, start))
        }
        character.limbs[limb].points[joint] = canvasToCharacterRelative(add(scale(frame / endFrame, diff), start), character)
    }
}

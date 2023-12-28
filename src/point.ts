export interface Point {
    x : number,
    y : number
}

export function dist (a : Point, b : Point) {
    let diffx = a.x - b.x
    let diffy = a.y - b.y
    return Math.sqrt(diffx * diffx + diffy * diffy)
}

export function add(a : Point, b : Point) : Point {
    return {x : a.x + b.x, y : a.y + b.y}
}

export function scale(s : number, a : Point) : Point {
    return {x : a.x * s, y : a.y * s}
}

export function pt(x : number, y : number): Point {
    return {x : x, y : y}
}

export function lerp(from : Point, to : Point, t : number): Point {
    let diff = add(to, scale(-1, from))
    return add(from, scale(t, diff))
}
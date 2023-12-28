import { Run } from "./anim"
import { Character } from "./character"
import { Obj } from "./obj"
import { Resources } from "./resources"

export type Tasks = (((frame: number) => void) | Run | Run[])[]
export interface World {
    res: Resources
    things: (Character | Obj)[],
    tasks: Tasks,
    fps: number
}

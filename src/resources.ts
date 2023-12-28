export class Resources {
    maxId : number
    idToValue : Map<number, string>
    valueToId : Map<string, number>
    
    constructor () {
        this.maxId = 0
        this.idToValue = new Map
        this.valueToId = new Map
    }

    add(resource : string) : number {
        let id = this.valueToId.get(resource)
        if (id !== undefined) {
            return id
        } else {
            this.idToValue.set(this.maxId, resource)
            this.valueToId.set(resource, this.maxId)
            this.maxId++
            return this.maxId - 1
        }
    }

    serialize() : [number, string][] {
        return Array.from(this.idToValue.entries())
    }
}
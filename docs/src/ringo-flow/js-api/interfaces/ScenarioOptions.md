# Interface: ScenarioOptions

## Properties

### only?

> `optional` **only?**: `boolean`

If any scenario sets `only: true`, only those run.

***

### skip?

> `optional` **skip?**: `string` \| `boolean`

`true` to skip, or a string reason (reported, not run).

***

### tags?

> `optional` **tags?**: `string`[]

Tags for filtering with `--tag` / `--exclude-tag`.

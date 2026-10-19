# <product> logs: what the shared skill doesn't know

Read by an agent after the `/thmsn-logs` skill. Keep it to what is
particular to this repo; the fleet conventions are in the skill.

## Services

- `dev.thmsn.<product>.server`: what it is, and where its pre-exporter stdout goes.

## Fields worth filtering on

Structured metadata beyond the fleet set (`severity_text`, `scope_name`, `api_operation`,
`status`, `duration_ms`, `outcome`, `trace_id`). Run `obsq.sh run build` or
`obsq.sh meta '{{sel}} |= `<message>`'` to see what a line carries.

## Known noise

Warnings that are expected and recovered from, so a sweep can skip them.

## Recipes

What each file in `queries/` and `commands/` answers.

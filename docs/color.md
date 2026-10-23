# Terminal color policy

## Scope

Color is a presentation concern of the FLON application and formatter options.
It is not a `json-skimmer` concern.

The initial semantic palette, CLI choice, terminal adaptation, and environment
precedence described below are implemented. Theme selection and custom role
colors remain future work.

FLON will use both crates in the `anstyle` ecosystem:

- `anstyle` defines reusable semantic styles and RGB colors.
- `anstream` adapts styled output to the destination and strips or preserves
  terminal sequences according to the resolved color choice.

The formatter should use semantic roles rather than embedding escape sequences
or color decisions throughout its path and value logic.

## Semantic palette

The initial palette follows `node-flon`:

| Role | Default family |
| --- | --- |
| Root binding | Blue |
| Object key | Blue |
| Array index | Orange |
| String value | Green |
| Number value | Orange |
| Boolean value | Purple |
| Null value | Yellow |
| Timing summary | Orange |
| Warning or error | Red |

The palette is data supplied to the renderer. Plain and styled output must share
the same transformation logic and differ only in presentation.

## Color choice

The CLI should expose:

```text
--color=auto|always|never
```

`--no-color` is an alias for `--color=never`.

- `auto` enables styling only when the destination and terminal capabilities
  support it.
- `always` preserves styling even when output is redirected.
- `never` emits plain output.

Automatic color must normally be disabled for pipes and files because FLON is
designed for line-oriented pipelines. The CLI resolves the destination before
formatting: redirected `auto` selects plain rendering, so ANSI bytes are never
generated or counted toward confirmed-output memory thresholds and candidate
limits. `always` and terminal `auto` retain styling, and their generated ANSI
bytes count toward those storage limits. `--show-config` reports formatter
styling resolved for the current stdout destination while retaining the
requested color policy.

## Environment conventions

FLON should recognize common conventions without claiming that they form one
universal standard:

- `NO_COLOR` disables color when present.
- `FORCE_COLOR` requests color and may carry a capability level.
- `CLICOLOR=0` requests no color.
- `CLICOLOR_FORCE` requests color.
- `TERM=dumb` indicates that automatic styling should be disabled.

The intended precedence is:

1. Explicit CLI `--color` or `--no-color`.
2. `FLON_COLOR=auto|always|never`.
3. `NO_COLOR`.
4. `FORCE_COLOR` or `CLICOLOR_FORCE`.
5. `CLICOLOR=0` or `TERM=dumb`.
6. Destination and terminal capability detection.

Within the force-color level, `FORCE_COLOR` is checked before
`CLICOLOR_FORCE`. Values that are empty or `0` do not force color. `anstream`
adapts the resolved output choice, but FLON owns this precedence contract
rather than assuming a dependency's implicit policy.

## Themes and customization

FLON should support:

```text
FLON_THEME=auto|light|dark
FLON_COLOR_ROOT=<color>
FLON_COLOR_KEY=<color>
FLON_COLOR_INDEX=<color>
FLON_COLOR_STRING=<color>
FLON_COLOR_NUMBER=<color>
FLON_COLOR_BOOLEAN=<color>
FLON_COLOR_NULL=<color>
FLON_COLOR_SUMMARY=<color>
FLON_COLOR_ERROR=<color>
```

The accepted color syntax must be defined before implementation. Hex RGB is the
minimum useful custom form. Theme selection establishes defaults; individual
role overrides are applied afterward.

For migration, the existing `BUZULI_COLOR_LIGHT`, `BUZULI_COLOR_DARK`, and
`BUZULI_COLOR_*` variables may be accepted as lower-priority aliases. New
FLON-specific names avoid unexpectedly changing unrelated applications.

Automatic light-versus-dark detection is often unreliable. If it cannot be
done predictably and portably, `auto` should use a neutral default palette and
leave explicit light or dark selection to the user.

## Capability levels

The renderer should degrade styles according to terminal capability:

- no color;
- basic ANSI colors;
- 256-color palette; or
- 24-bit RGB.

Exact RGB fidelity is less important than legibility and consistent semantic
roles. Tests should verify plain output exactly and styled output structurally,
without depending on the test runner being attached to a terminal.

## Future shared library

Color policy should initially remain inside FLON. A separate Rust equivalent of
`@buzuli/color` becomes worthwhile only after another application needs the
same palette, environment precedence, theme handling, and capability fallback.

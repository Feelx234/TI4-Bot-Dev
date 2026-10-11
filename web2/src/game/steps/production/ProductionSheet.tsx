import type { BuildCountView, BuildGroupView, BuildSheetView, BuildUnitView } from "../../../model";
import { plural } from "../../../model";
import { Button, Counter, Gauge, Icon, cx } from "../../../ui";
import { useDispatch } from "../../context";

// The columns of a unit row: name, cost, combat, move, capacity, left in supply, counter. On a
// phone the values are a second line with their words, so nothing is cut at the side.
const COLUMNS =
  "grid grid-cols-[minmax(0,1fr)_52px_50px_36px_58px_30px_92px] items-center gap-x-1.5 px-3 phone:grid-cols-[minmax(0,1fr)_auto] phone:px-2.5";
const STATS = ["Cost", "Combat", "Move", "Capacity", "Left"];

/** The counter of one unit in one place. One step is one purchase; it shows the units. */
function BuildCounter({ count }: { count: BuildCountView }) {
  const dispatch = useDispatch();
  return (
    <span className="justify-self-end" title={count.reason ?? undefined}>
      <Counter
        {...count.counter}
        shown={count.made}
        onChange={(value) => dispatch({ type: "setCount", key: count.counter.id, value })}
      />
    </span>
  );
}

/** One kind of unit as printed, with its counter. A unit that is not produced here is dimmed. */
function UnitRow({ unit, byPlace }: { unit: BuildUnitView; byPlace: boolean }) {
  const off = !!unit.unavailable;
  const values = [unit.price, unit.combat, unit.move, unit.capacity, unit.left ?? "–"];
  return (
    <>
      <div className={cx(COLUMNS, "min-h-8 border-t border-line/50 touch:min-h-10")}>
        <span
          className={cx("flex min-w-0 items-center gap-1.5", off && "opacity-45")}
          title={[unit.name, unit.abilities].filter(Boolean).join(" · ")}
        >
          <Icon name={unit.unit} className="size-4 flex-none text-icon" />
          <strong className="truncate font-mid">{unit.name}</strong>
        </span>
        <span
          className={cx(
            "contents text-xs tabular-nums phone:order-last phone:col-span-2 phone:flex phone:flex-wrap phone:gap-x-3 phone:pb-1",
            off && "*:opacity-45",
          )}
        >
          {values.map((value, index) => (
            <span key={STATS[index]} className="whitespace-nowrap">
              <span className="hidden text-muted phone:inline">{STATS[index]} </span>
              {value}
            </span>
          ))}
        </span>
        {/* A word, not only the dimmed row: why the unit is not produced here. */}
        {unit.unavailable ? (
          <span
            className="justify-self-end text-xs whitespace-nowrap text-muted"
            title={unit.unavailable.reason}
          >
            {unit.unavailable.word}
          </span>
        ) : byPlace ? (
          <span />
        ) : (
          unit.counts.map((count) => <BuildCounter key={count.counter.id} count={count} />)
        )}
      </div>
      {byPlace &&
        unit.counts.map((count) => (
          <div
            key={count.counter.id}
            className="flex min-h-8 items-center justify-between gap-2 pr-3 pl-[38px] text-xs text-muted touch:min-h-10 phone:pr-2.5"
          >
            {count.place}
            <BuildCounter count={count} />
          </div>
        ))}
    </>
  );
}

/** The ships, or the ground forces: a heading and one row for each kind of unit. */
function Group({ group, first }: { group: BuildGroupView; first: boolean }) {
  return (
    <div role="group" aria-label={group.title}>
      <div
        className={cx(
          COLUMNS,
          "min-h-6 bg-raised/50 text-xs text-muted",
          !first && "border-t border-line/50",
        )}
      >
        <span className="truncate">{group.title}</span>
        {/* The words of the columns are said once. On a phone each value has its word. */}
        {first && (
          <span className="contents phone:hidden">
            {STATS.map((word) => (
              <span key={word}>{word}</span>
            ))}
          </span>
        )}
      </div>
      {group.units.map((unit) => (
        <UnitRow key={unit.id} unit={unit} byPlace={group.byPlace} />
      ))}
    </div>
  );
}

/** "3 units · 5 resources", with what the discount took off. */
function Bill({ view }: { view: BuildSheetView }) {
  if (view.made === 0) {
    return <>Nothing is produced</>;
  }
  return (
    <>
      <strong className="font-mid text-text">
        {plural(view.made, "unit")} · {plural(view.cost, "resource")}
      </strong>
      {view.discount > 0 && ` · discount ${view.discount}`}
    </>
  );
}

/**
 * The build of a production with its controls, on the board, at the system that produces. On a
 * phone it is a sheet over the lower part of the map. The payment follows, on the map.
 */
export function ProductionSheet({ view }: { view: BuildSheetView }) {
  const dispatch = useDispatch();
  return (
    <div
      role="group"
      aria-label={`Produce in ${view.label}`}
      className="absolute bottom-2.5 left-2.5 z-2 max-h-[62%] w-[540px] overflow-auto rounded-lg border border-line bg-surface/97 text-sm phone:inset-x-0 phone:bottom-0 phone:z-3 phone:max-h-[70%] phone:w-auto phone:rounded-none phone:border-x-0 phone:border-b-0 phone:bg-surface"
    >
      <div className="sticky top-0 z-1 bg-surface">
        <div className="flex items-center gap-2 border-b border-line py-0.5 pr-1 pl-3 phone:pl-2.5">
          <h3 className="truncate text-base font-semibold" title={view.label}>
            {view.label}
          </h3>
          <span role="status" className="min-w-0 flex-1 truncate text-xs text-muted">
            <Bill view={view} />
          </span>
          <Button
            size="sm"
            tone="quiet"
            disabled={view.made === 0}
            onClick={() => dispatch({ type: "resetOrigin", system: view.system })}
          >
            Reset
          </Button>
          {/* Closes the sheet. With something to pay, the map is then the payment. */}
          <button
            type="button"
            aria-label="Done with the build"
            title={
              view.made > 0 && view.cost > 0
                ? "Done with the build · the payment is next"
                : "Done with the build · closes the sheet"
            }
            className="grid size-7 flex-none place-items-center rounded-md border border-line-strong text-icon hover:bg-raised touch:size-9"
            onClick={() => dispatch({ type: "markOrigin", system: view.system })}
          >
            <Icon name="check" />
          </button>
        </div>
        <div className="grid grid-cols-2 gap-x-4 gap-y-1.5 border-b border-line px-3 py-1.5 phone:px-2.5">
          {view.gauges.map((gauge) => (
            <Gauge key={gauge.label} {...gauge} />
          ))}
        </div>
      </div>
      {view.groups.map((group, index) => (
        <Group key={group.title} group={group} first={index === 0} />
      ))}
    </div>
  );
}

/** In the panel: what is produced in each place, and the limits. The controls are on the board. */
export function ProductionSummary({ view }: { view: BuildSheetView }) {
  const dispatch = useDispatch();
  const row =
    "row flex min-h-8 w-full flex-wrap items-baseline gap-x-2.5 px-3.5 py-1 text-left hover:bg-raised [.row+&]:border-t [.row+&]:border-line/50 phone:px-2.5";
  return (
    <div className="flex flex-col gap-3">
      <div className="rounded-lg border border-line">
        <div className="border-b border-line px-3.5 py-1.5 text-xs text-muted phone:px-2.5">
          {view.stage === "build" ? (
            "Choose the units on the map, at the system that produces."
          ) : (
            <>
              Choose on the map what pays. <Bill view={view} />
            </>
          )}
        </div>
        {view.places.map((place) => (
          <button
            key={place.id}
            type="button"
            className={row}
            onClick={() => dispatch({ type: "inspectSystem", system: view.system })}
          >
            <strong className="font-mid">{place.name}</strong>
            <span className="text-xs text-muted">
              {place.builds ? (
                <strong className="font-mid text-text">{place.builds}</strong>
              ) : (
                "Nothing is produced"
              )}
            </span>
          </button>
        ))}
      </div>
      <div className="grid grid-cols-[repeat(auto-fit,minmax(150px,1fr))] gap-x-4 gap-y-3">
        {view.gauges.map((gauge) => (
          <Gauge key={gauge.label} {...gauge} />
        ))}
      </div>
    </div>
  );
}

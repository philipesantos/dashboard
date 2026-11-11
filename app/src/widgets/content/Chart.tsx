// A chart the host draws as inline SVG elements, which load nothing: bars or lines, series in the
// chart-1 to chart-5 roles in order, and a text table of the values for assistive technology
// (specs/009-declarative-widget-ui/research.md, R10).

import type { Series } from "../api";

const WIDTH = 320;
const HEIGHT = 140;
const PAD = 8;

const stroke = ["stroke-chart-1", "stroke-chart-2", "stroke-chart-3", "stroke-chart-4", "stroke-chart-5"];
const fill = ["fill-chart-1", "fill-chart-2", "fill-chart-3", "fill-chart-4", "fill-chart-5"];
const swatch = ["bg-chart-1", "bg-chart-2", "bg-chart-3", "bg-chart-4", "bg-chart-5"];

type ChartProps = { chart: { kind: string; labels: string[]; series: Series[] } };

export function Chart({ chart }: ChartProps) {
  const values = chart.series.flatMap((series) => series.values);
  const low = Math.min(0, ...values);
  const high = Math.max(0, ...values);
  const span = high - low || 1;
  const y = (value: number) => PAD + ((high - value) / span) * (HEIGHT - 2 * PAD);
  const count = Math.max(chart.labels.length, 1);
  const step = (WIDTH - 2 * PAD) / count;
  return (
    <figure className="flex flex-col gap-2 text-xs text-muted-foreground">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} className="h-36 w-full" aria-hidden preserveAspectRatio="none">
        <line x1={PAD} x2={WIDTH - PAD} y1={y(0)} y2={y(0)} className="stroke-border" strokeWidth={1} />
        {chart.kind === "bar"
          ? chart.series.map((series, s) => {
              const width = (step * 0.8) / chart.series.length;
              return series.values.map((value, i) => (
                <rect
                  key={`${s}-${i}`}
                  x={PAD + i * step + step * 0.1 + s * width}
                  y={Math.min(y(value), y(0))}
                  width={Math.max(width - 1, 1)}
                  height={Math.abs(y(value) - y(0))}
                  className={fill[s]}
                />
              ));
            })
          : chart.series.map((series, s) => (
              <polyline
                key={s}
                points={series.values.map((value, i) => `${PAD + (i + 0.5) * step},${y(value)}`).join(" ")}
                fill="none"
                strokeWidth={2}
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
                className={stroke[s]}
              />
            ))}
      </svg>
      <div className="flex justify-between gap-1" aria-hidden>
        {chart.labels.map((label, index) => (
          <bdi key={index} className="min-w-0 truncate">
            {label}
          </bdi>
        ))}
      </div>
      <figcaption className="flex flex-wrap gap-3">
        {chart.series.map((series, s) => (
          <span key={s} className="flex items-center gap-1.5">
            <span aria-hidden className={`size-2.5 rounded-sm ${swatch[s]}`} />
            <bdi>{series.name}</bdi>
          </span>
        ))}
      </figcaption>
      <table className="sr-only">
        <thead>
          <tr>
            <th />
            {chart.series.map((series, s) => (
              <th key={s}>
                <bdi>{series.name}</bdi>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {chart.labels.map((label, i) => (
            <tr key={i}>
              <th>
                <bdi>{label}</bdi>
              </th>
              {chart.series.map((series, s) => (
                <td key={s}>{series.values[i]}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}

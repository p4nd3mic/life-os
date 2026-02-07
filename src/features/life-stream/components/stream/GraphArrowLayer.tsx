type GraphArrowPath = {
  id: string;
  d: string;
  headD?: string;
  dimmed?: boolean;
  isTrunk?: boolean;
};

type GraphArrowLayerProps = {
  paths: GraphArrowPath[];
  gradientId: string;
  orientation?: "horizontal" | "vertical";
};

export function GraphArrowLayer({
  paths,
  gradientId,
  orientation = "horizontal",
}: GraphArrowLayerProps) {
  const isVertical = orientation === "vertical";
  return (
    <svg
      className="life-causal-card__links"
      aria-hidden="true"
      role="presentation"
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient
          id={gradientId}
          x1="0%"
          y1="0%"
          x2={isVertical ? "0%" : "100%"}
          y2={isVertical ? "100%" : "0%"}
        >
          <stop offset="0%" stopColor="rgba(251, 191, 36, 0.95)" />
          <stop offset="60%" stopColor="rgba(245, 208, 90, 0.95)" />
          <stop offset="100%" stopColor="rgba(110, 231, 183, 0.92)" />
        </linearGradient>
      </defs>

      {paths.map((path) => (
        <g key={path.id}>
          <path
            d={path.d}
            className={`life-causal-card__path${path.dimmed ? " is-dimmed" : ""}${
              path.isTrunk ? " is-trunk" : ""
            }`}
            style={{ stroke: `url(#${gradientId})` }}
          />
          {path.headD ? (
            <path
              d={path.headD}
              className={`life-causal-card__head${path.dimmed ? " is-dimmed" : ""}`}
            />
          ) : null}
        </g>
      ))}
    </svg>
  );
}

export type { GraphArrowPath };

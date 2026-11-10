// A fixture for the usage scan. Not compiled.
export function Widget() {
  return (
    <div className="bg-card text-foreground ring-ring/50 dark:hover:bg-input/50 disabled:opacity-50">
      <div className="bg-black/10 text-red-500 bg-[#fff] bg-transparent border-transparent text-current" />
      <div className="text-sm ring-3 shadow-md outline-none border-0 text-right ring-[3px] text-[0.8rem]" />
      <div className="hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] [&_svg:not([class*='size-'])]:size-4" />
    </div>
  );
}

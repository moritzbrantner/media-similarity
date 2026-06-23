export function RegistryMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2">
      <dt className="text-xs font-semibold text-neutral-500">{label}</dt>
      <dd className="mt-1 text-2xl font-semibold text-neutral-950">{value}</dd>
    </div>
  );
}

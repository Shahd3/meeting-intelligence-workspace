type PlaceholderProps = { title: string; description: string };

export function Placeholder({ title, description }: PlaceholderProps) {
  return (
    <section className="placeholder" aria-label={title}>
      <span className="placeholder-mark" aria-hidden="true">✦</span>
      <h2>{title}</h2>
      <p>{description}</p>
      <span className="badge">Coming soon</span>
    </section>
  );
}

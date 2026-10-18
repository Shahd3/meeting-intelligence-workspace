export function RecordingWaveform({ peak }: { peak: number }) {
  return (
    <div className="wave" aria-hidden="true" style={{ opacity: peak > 0.005 ? 1 : 0.35 }}>
      {Array.from({ length: 40 }, (_, index) => <i key={index} />)}
    </div>
  );
}

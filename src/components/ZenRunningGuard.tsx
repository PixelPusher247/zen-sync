interface Props {
  action: string;
}

export default function ZenRunningGuard({ action }: Props) {
  return (
    <div className="flex flex-col items-center gap-3 p-5 bg-warning/5 border border-warning/20 rounded-2xl text-center animate-fade-in">
      <div className="w-10 h-10 rounded-full bg-warning/10 flex items-center justify-center text-xl">
        ⚠
      </div>
      <div>
        <p className="font-semibold text-white text-sm">
          Close Zen Browser first
        </p>
        <p className="text-muted text-xs mt-1 leading-relaxed">
          Zen Browser is currently running. Close it completely before{" "}
          {action} to avoid data corruption.
        </p>
      </div>
    </div>
  );
}

import { cn } from '@/lib/utils';
import type { LucideIcon } from 'lucide-react';

interface StatCardProps {
  icon: LucideIcon;
  label: string;
  value: string;
  sub?: string;
  colorClass?: string;
  accentClass?: string;
}

export default function StatCard({ icon: Icon, label, value, sub, colorClass, accentClass }: StatCardProps) {
  return (
    <div className={cn(
      "bg-bg-card border border-border rounded-lg p-2.5 hover:border-accent/50 transition-all duration-200 hover:-translate-y-0.5",
      colorClass
    )}>
      <div className="flex items-center gap-1 mb-0.5">
        <Icon className="w-2.5 h-2.5 text-text-3" />
        <span className="text-[9px] text-text-3 uppercase tracking-wider font-semibold">{label}</span>
      </div>
      <div className={cn("text-xl font-bold font-mono leading-tight", accentClass)}>{value}</div>
      {sub && <div className="text-[10px] text-text-2 mt-0.5 font-mono">{sub}</div>}
    </div>
  );
}

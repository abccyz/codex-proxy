import { memo, useState, useRef, useMemo, useCallback, useEffect } from 'react';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { cn, formatNumber, formatLatency } from '@/lib/utils';

interface FloatingWidgetProps {
  /** 上游模型名（仅在嵌入主窗口时需要；widget 窗口内部自行获取） */
  upstreamModel: string;
  /** 是否为独立 widget 窗口模式（无 fixed 定位、无拖拽、无 widgetVisible 检查） */
  inWidgetWindow?: boolean;
}

/**
 * 胶囊卡片悬浮窗 — 展示代理服务实时工作状态。
 *
 * 两种运行模式：
 * - 嵌入主窗口（inWidgetWindow=false）：fixed 定位、支持拖拽、受 widgetVisible 控制
 * - 独立 widget 窗口（inWidgetWindow=true）：窗口即组件、OS 级拖拽、widgetVisible 不适用
 */
const FloatingWidget = memo(
  function FloatingWidget({ upstreamModel, inWidgetWindow = false }: FloatingWidgetProps) {
    const { proxyRunning, widgetVisible } = useApp();
    const { snapshot } = useMetrics();

    const [expanded, setExpanded] = useState(false);
    const [position, setPosition] = useState({ x: 0, y: 0 });
    // widget 窗口模式下自行获取的模型名
    const [widgetModel, setWidgetModel] = useState('');

    // 拖拽相关 ref（仅 in-app 模式使用）
    const isDragging = useRef(false);
    const posStart = useRef({ x: 0, y: 0 });
    const dragStart = useRef({ x: 0, y: 0 });
    const leaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    // widget 窗口模式：自行获取上游模型名
    useEffect(() => {
      if (!inWidgetWindow) return;
      let cancelled = false;
      const fetchModel = async () => {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          const info = await invoke<{ model: string }>('get_upstream_info');
          if (!cancelled) setWidgetModel(info.model);
        } catch { /* ignore */ }
      };
      fetchModel();
      const interval = setInterval(fetchModel, 5000);
      return () => { cancelled = true; clearInterval(interval); };
    }, [inWidgetWindow]);

    // 实际使用的模型名
    const effectiveModel = inWidgetWindow ? widgetModel : upstreamModel;

    // ---- 派生数据（useMemo 避免每次渲染重复计算） ----
    const derived = useMemo(() => {
      if (!snapshot) return null;
      const ratio = snapshot.total_tokens > 0
        ? snapshot.total_output_tokens / snapshot.total_tokens
        : 0;
      const ringRatio = Math.max(0.05, Math.min(ratio, 1));
      const displayModel = snapshot.live_stream?.model || effectiveModel || '--';
      return {
        tokenDisplay: formatNumber(snapshot.total_tokens),
        callCount: formatNumber(snapshot.total),
        avgLatency: formatLatency(snapshot.avg_latency * 1000),
        modelName: displayModel,
        ringRatio,
      };
    }, [snapshot, effectiveModel]);

    // ---- 拖拽处理（仅 in-app 模式） ----
    const onPointerDown = useCallback((e: React.PointerEvent) => {
      if (inWidgetWindow) return;
      isDragging.current = false;
      dragStart.current = { x: e.clientX, y: e.clientY };
      posStart.current = { ...position };
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    }, [inWidgetWindow, position]);

    const onPointerMove = useCallback((e: React.PointerEvent) => {
      if (inWidgetWindow || e.buttons !== 1) return;
      const dx = e.clientX - dragStart.current.x;
      const dy = e.clientY - dragStart.current.y;
      if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
        isDragging.current = true;
      }
      setPosition({
        x: posStart.current.x + dx,
        y: posStart.current.y + dy,
      });
    }, [inWidgetWindow]);

    const onPointerUp = useCallback(() => {
      if (inWidgetWindow) return;
      setPosition(prev => {
        const maxX = window.innerWidth - 220;
        const maxY = window.innerHeight - 100;
        return {
          x: Math.max(-16, Math.min(prev.x, maxX)),
          y: Math.max(-16, Math.min(prev.y, maxY)),
        };
      });
    }, [inWidgetWindow]);

    const handleClick = useCallback(() => {
      if (inWidgetWindow || !isDragging.current) {
        setExpanded(v => !v);
      }
    }, [inWidgetWindow]);

    // hover 交互
    const handleMouseEnter = useCallback(() => {
      if (leaveTimer.current) {
        clearTimeout(leaveTimer.current);
        leaveTimer.current = null;
      }
      setExpanded(true);
    }, []);

    const handleMouseLeave = useCallback(() => {
      leaveTimer.current = setTimeout(() => setExpanded(false), 200);
    }, []);

    // ---- 可见性判断 ----
    // in-app 模式：代理未运行 或 用户关闭悬浮窗 → 隐藏
    if (!inWidgetWindow && (!proxyRunning || !widgetVisible)) return null;
    // widget 窗口模式：代理未运行 → 显示简略占位
    if (inWidgetWindow && !proxyRunning) {
      return (
        <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-card/60 backdrop-blur border border-border rounded-full">
          <div className="w-2 h-2 rounded-full bg-text-3" />
          <span className="text-[10px] text-text-3 font-mono">--</span>
        </div>
      );
    }

    // ---- 快照未就绪：渲染占位胶囊 ----
    if (!derived) {
      const Placeholder = (
        <div className="flex items-center gap-3 bg-bg-card/80 backdrop-blur border border-border rounded-full px-3 py-2 shadow-lg">
          <div className="w-4 h-4 rounded-full border-2 border-border animate-pulse" />
          <span className="text-xs text-text-3 font-mono">--</span>
        </div>
      );
      if (inWidgetWindow) return Placeholder;
      return (
        <div
          className="fixed bottom-4 right-4 z-40 select-none"
          style={{ transform: `translate(${position.x}px, ${position.y}px)` }}
        >
          {Placeholder}
        </div>
      );
    }

    // ---- SVG 进度环参数 ----
    const R = expanded ? 18 : 13;
    const svgSize = expanded ? 48 : 34;
    const circumference = 2 * Math.PI * R;
    const dashOffset = circumference * (1 - derived.ringRatio);
    const center = svgSize / 2;

    // ---- 折叠态模型名截断 ----
    const shortModel = derived.modelName.length > 12
      ? derived.modelName.slice(0, 12) + '...'
      : derived.modelName;

    // ---- 公共内层卡片 ----
    const card = (
      <div
        className={cn(
          'bg-bg-card/90 backdrop-blur-md border border-border shadow-xl',
          'transition-all duration-300 ease-out',
          expanded
            ? 'rounded-xl p-3 min-w-[210px]'
            : 'rounded-full px-3 py-2 flex items-center gap-2.5',
        )}
      >
        {/* ====== 左侧：Token 进度环 ====== */}
        <div
          className={cn(
            'flex-shrink-0 flex items-center justify-center',
            expanded ? 'mr-2' : '',
          )}
        >
          <svg
            width={svgSize}
            height={svgSize}
            viewBox={`0 0 ${svgSize} ${svgSize}`}
            className="flex-shrink-0"
          >
            <circle
              cx={center} cy={center} r={R} fill="none"
              stroke="var(--border)" strokeWidth={expanded ? 3 : 2.5}
            />
            <circle
              cx={center} cy={center} r={R} fill="none"
              stroke="var(--accent)" strokeWidth={expanded ? 3 : 2.5}
              strokeLinecap="round"
              strokeDasharray={circumference}
              strokeDashoffset={dashOffset}
              transform={`rotate(-90 ${center} ${center})`}
              style={{ transition: 'stroke-dashoffset 0.6s ease' }}
            />
            <text
              x={center} y={center}
              textAnchor="middle" dominantBaseline="central"
              fill="var(--text-1)"
              fontSize={expanded ? 9 : 8}
              fontWeight="bold" fontFamily="monospace"
            >
              {derived.tokenDisplay}
            </text>
          </svg>
        </div>

        {/* ====== 右侧：模型信息 + 调用统计 ====== */}
        <div className={cn('flex flex-col min-w-0', expanded ? 'gap-1.5' : 'gap-0.5')}>
          <span
            className={cn(
              'font-mono truncate',
              expanded ? 'text-xs font-semibold text-text-1' : 'text-[11px] text-text-2',
            )}
          >
            {expanded ? derived.modelName : shortModel}
          </span>
          <div
            className={cn(
              'flex items-center gap-2 text-text-3 font-mono',
              expanded ? 'text-[11px]' : 'text-[10px]',
              !expanded && 'hidden',
            )}
          >
            <span>{derived.callCount} calls</span>
            <span className="text-border">·</span>
            <span>{derived.avgLatency}</span>
          </div>
        </div>

        {/* 折叠态下的简要数据行 */}
        {!expanded && (
          <span className="text-[10px] text-text-3 font-mono whitespace-nowrap">
            {derived.callCount} · {derived.avgLatency}
          </span>
        )}
      </div>
    );

    // ---- widget 窗口模式：直接渲染卡片（无外层容器） ----
    if (inWidgetWindow) {
      return (
        <div
          className="flex items-center justify-center w-full h-full"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
          onClick={handleClick}
        >
          {card}
        </div>
      );
    }

    // ---- in-app 模式：fixed 定位 + 拖拽 ----
    return (
      <div
        className={cn(
          'fixed bottom-4 right-4 z-40 select-none',
          'transition-all duration-300 ease-out',
          expanded ? 'cursor-default' : 'cursor-grab active:cursor-grabbing',
        )}
        style={{ transform: `translate(${position.x}px, ${position.y}px)` }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onClick={handleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {card}
      </div>
    );
  },
  (prev, next) =>
    prev.upstreamModel === next.upstreamModel &&
    prev.inWidgetWindow === next.inWidgetWindow,
);

export default FloatingWidget;

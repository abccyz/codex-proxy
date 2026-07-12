import { useState, useMemo, useCallback } from 'react';
import { Search, ChevronLeft, ChevronRight, X, FileText } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { t } from '@/lib/i18n';
import { cn, formatLatency } from '@/lib/utils';
import { invoke } from '@tauri-apps/api/core';
import type { InputDetail } from '@/lib/types';

export default function HistoryPage() {
  const { lang } = useApp();
  const { snapshot } = useMetrics();
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(0);
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null);
  const [selectedDetail, setSelectedDetail] = useState<InputDetail | null>(null);
  const pageSize = 15;

  const filtered = useMemo(() => {
    if (!snapshot) return [];
    const q = search.toLowerCase();
    return [...snapshot.history].reverse().filter(r => !q || r.model.toLowerCase().includes(q) || r.preview.toLowerCase().includes(q));
  }, [snapshot, search]);

  const paged = useMemo(() => filtered.slice(page * pageSize, (page + 1) * pageSize), [filtered, page]);
  const totalPages = Math.max(1, Math.ceil(filtered.length / pageSize));

  const openDetail = useCallback(async (idx: number) => {
    setSelectedIdx(idx);
    try {
      const detail = await invoke<InputDetail>('get_history_detail', { idx });
      setSelectedDetail(detail);
    } catch { setSelectedDetail(null); }
  }, []);

  return (
    <div className="h-full flex flex-col p-4 overflow-hidden">
      {/* Search */}
      <div className="flex items-center gap-2 mb-3 flex-shrink-0">
        <div className="relative flex-1 max-w-xs">
          <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-text-3" />
          <input
            value={search}
            onChange={e => { setSearch(e.target.value); setPage(0); }}
            placeholder={t(lang, 'history_search')}
            className="w-full pl-7 pr-3 py-1.5 bg-bg-input border border-border rounded-md text-xs text-text-1 placeholder:text-text-3 focus:outline-none focus:border-accent transition-colors"
          />
        </div>
        <span className="text-[10px] text-text-3">{filtered.length} records</span>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto rounded-lg border border-border">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-bg-elev z-10">
            <tr className="text-text-3 uppercase tracking-wider text-[10px]">
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_time')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_model')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_status')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_latency')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_tokens')}</th>
              <th className="text-left px-3 py-2 font-semibold">Preview</th>
            </tr>
          </thead>
          <tbody>
            {paged.map((r, i) => (
              <tr
                key={`${r.time}-${i}`}
                onClick={() => openDetail(page * pageSize + i)}
                className={cn(
                  "border-t border-border hover:bg-blue-bg/20 cursor-pointer transition-colors",
                  selectedIdx === page * pageSize + i && "bg-blue-bg/40"
                )}
              >
                <td className="px-3 py-1.5 font-mono text-text-2 whitespace-nowrap">{r.time}</td>
                <td className="px-3 py-1.5 font-mono text-text-1 max-w-[120px] truncate">{r.model}</td>
                <td className="px-3 py-1.5">
                  <span className={cn(
                    "px-1.5 py-0.5 rounded text-[10px] font-semibold",
                    r.status === 'success' ? "bg-green-bg text-green" : "bg-red-bg text-red"
                  )}>
                    {r.status === 'success' ? 'OK' : 'ERR'}
                  </span>
                </td>
                <td className="px-3 py-1.5 font-mono text-text-2">{formatLatency(r.latency * 1000)}</td>
                <td className="px-3 py-1.5 font-mono text-text-2">
                  {r.input_tokens}/{r.output_tokens}
                </td>
                <td className="px-3 py-1.5 text-text-2 max-w-[250px] truncate">{r.preview}</td>
              </tr>
            ))}
            {filtered.length === 0 && (
              <tr>
                <td colSpan={6} className="px-3 py-10 text-center text-text-3">
                  {search ? t(lang, 'history_no_results') : t(lang, 'history_empty')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex items-center justify-between pt-2 flex-shrink-0">
        <span className="text-[10px] text-text-3">
          {page * pageSize + 1}–{Math.min((page + 1) * pageSize, filtered.length)} / {filtered.length}
        </span>
        <div className="flex items-center gap-1">
          <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0}
            className="p-1 rounded text-text-3 hover:text-text-1 disabled:opacity-30">
            <ChevronLeft className="w-3.5 h-3.5" />
          </button>
          <span className="text-[10px] text-text-3 font-mono">{page + 1}/{totalPages}</span>
          <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))} disabled={page >= totalPages - 1}
            className="p-1 rounded text-text-3 hover:text-text-1 disabled:opacity-30">
            <ChevronRight className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Detail panel */}
      {selectedDetail && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => { setSelectedIdx(null); setSelectedDetail(null); }}>
          <div className="bg-bg-card border border-border rounded-lg w-[700px] max-h-[80vh] flex flex-col shadow-2xl" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0">
              <div className="flex items-center gap-2">
                <FileText className="w-4 h-4 text-accent" />
                <span className="text-sm font-semibold">{t(lang, 'detail_title')}</span>
              </div>
              <button onClick={() => { setSelectedIdx(null); setSelectedDetail(null); }}
                className="p-1 rounded hover:bg-bg-elev text-text-2 hover:text-text-1">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="flex-1 overflow-auto p-4 space-y-3">
              {selectedDetail.instructions && (
                <div>
                  <div className="text-[10px] text-text-3 uppercase mb-1 font-semibold">{t(lang, 'detail_instructions')}</div>
                  <pre className="text-xs font-mono text-text-2 bg-bg-input p-2 rounded whitespace-pre-wrap max-h-40 overflow-auto">{selectedDetail.instructions}</pre>
                </div>
              )}
              {selectedDetail.messages.length > 0 && (
                <div>
                  <div className="text-[10px] text-text-3 uppercase mb-1 font-semibold">{t(lang, 'detail_messages')} ({selectedDetail.messages.length})</div>
                  <div className="space-y-1.5 max-h-60 overflow-auto">
                    {selectedDetail.messages.map((m, i) => (
                      <div key={i} className="bg-bg-input p-2 rounded text-xs">
                        <span className="text-accent font-semibold">{m.role}</span>
                        <pre className="text-text-2 mt-0.5 whitespace-pre-wrap">{m.content || '(empty)'}</pre>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {selectedDetail.tools && selectedDetail.tools !== '[]' && (
                <div>
                  <div className="text-[10px] text-text-3 uppercase mb-1 font-semibold">{t(lang, 'detail_tools')}</div>
                  <pre className="text-xs font-mono text-text-2 bg-bg-input p-2 rounded whitespace-pre-wrap max-h-40 overflow-auto">{selectedDetail.tools}</pre>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

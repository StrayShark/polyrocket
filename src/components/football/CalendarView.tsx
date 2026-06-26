/**
 * CalendarView —— 月视图日历,按 close date 展示 football 比赛。
 *
 * 渲染 7 列的日历网格,每个日期单元最多显示 2 个
 * 比赛 chip(队名缩写 + edge %)。支持通过 prev/next
 * 按钮切换月份。今天的日期用 accent 边框高亮。
 * 渲染在 Dashboard 底部。
 *
 * @param initialYear - 可选起始年份(默认当前)。
 * @param initialMonth - 可选起始月份 1-12(默认当前)。
 */
import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { marketCalendar, type CalendarDay } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { useT } from '@/lib/i18n';

/** 星期表头标签,以周一开始。 */
const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
/** 完整的月份名称,用于导航头部。 */
const MONTH_NAMES = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

interface CalendarViewProps {
  /** 可选起始年/月。默认当前。 */
  initialYear?: number;
  initialMonth?: number; // 1-12
}

export function CalendarView({ initialYear, initialMonth }: CalendarViewProps) {
  const { t } = useT();
  const now = new Date();
  const [year, setYear] = useState(initialYear ?? now.getFullYear());
  const [month, setMonth] = useState(initialMonth ?? (now.getMonth() + 1));

  const { data, isLoading, error } = useQuery({
    queryKey: ['market-calendar', year, month],
    queryFn: () => marketCalendar(year, month),
    staleTime: 5 * 60_000,
  });

  const fixturesByDate = useMemo(() => {
    const map = new Map<string, CalendarDay>();
    data?.forEach((d) => map.set(d.date, d));
    return map;
  }, [data]);

  const daysInMonth = new Date(year, month, 0).getDate();
  const firstDay = new Date(year, month - 1, 1);
  const startWeekday = (firstDay.getDay() + 6) % 7; // Mon=0 表示周一

  const prevMonth = () => {
    if (month === 1) { setMonth(12); setYear(year - 1); }
    else setMonth(month - 1);
  };
  const nextMonth = () => {
    if (month === 12) { setMonth(1); setYear(year + 1); }
    else setMonth(month + 1);
  };

  if (isLoading) {
    return (
      <Card title={t('calendar.title')}>
        <Skeleton className="h-48" />
      </Card>
    );
  }
  if (error) return <ErrorState message={String(error)} />;

  const cells: (number | null)[] = [];
  for (let i = 0; i < startWeekday; i++) cells.push(null);
  for (let d = 1; d <= daysInMonth; d++) cells.push(d);

  return (
    <Card
      title={t('calendar.title')}
      action={
        <div className="flex items-center gap-2">
          <button onClick={prevMonth} className="p-1 rounded hover:bg-border" style={{ color: 'var(--muted)' }}>
            <ChevronLeft className="w-4 h-4" />
          </button>
          <span className="text-body-sm font-medium" style={{ color: 'var(--fg)' }}>
            {MONTH_NAMES[month - 1]} {year}
          </span>
          <button onClick={nextMonth} className="p-1 rounded hover:bg-border" style={{ color: 'var(--muted)' }}>
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      }
    >
      <div className="grid grid-cols-7 gap-1">
        {WEEKDAYS.map((wd) => (
          <div key={wd} className="text-center text-[10px] font-medium py-1" style={{ color: 'var(--muted)' }}>
            {wd}
          </div>
        ))}
        {cells.map((day, i) => {
          if (day === null) return <div key={`e${i}`} />;
          const dateStr = `${year}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
          const dayData = fixturesByDate.get(dateStr);
          const hasFixtures = dayData && dayData.fixtures.length > 0;
          const today = new Date();
          const isToday =
            day === today.getDate() &&
            month === today.getMonth() + 1 &&
            year === today.getFullYear();

          return (
            <div
              key={day}
              className="min-h-[64px] rounded border p-1 text-[10px]"
              style={{
                borderColor: isToday ? 'var(--accent)' : 'var(--border)',
                background: hasFixtures ? 'var(--surface)' : 'transparent',
              }}
            >
              <div className="font-mono" style={{ color: isToday ? 'var(--accent)' : 'var(--muted)' }}>
                {day}
              </div>
              {hasFixtures && dayData.fixtures.slice(0, 2).map((f, fi) => (
                <Link
                  key={fi}
                  to={`/markets/${f.market_id}`}
                  className="block mt-1 rounded px-1 py-0.5 hover:opacity-80"
                  style={{ background: 'var(--accent)', color: 'white' }}
                  title={`${f.home} vs ${f.away} · ${f.time}`}
                >
                  <span className="font-mono">{f.home.slice(0, 3)} vs {f.away.slice(0, 3)}</span>
                  {f.edge != null && (
                    <span className="ml-1 opacity-80">{f.edge > 0 ? '+' : ''}{(f.edge * 100).toFixed(1)}%</span>
                  )}
                </Link>
              ))}
              {hasFixtures && dayData.fixtures.length > 2 && (
                <div className="mt-0.5" style={{ color: 'var(--muted)' }}>
                  +{dayData.fixtures.length - 2} more
                </div>
              )}
            </div>
          );
        })}
      </div>
      {(!data || data.length === 0) && (
        <p className="text-center text-[11px] py-4" style={{ color: 'var(--muted)' }}>
          {t('calendar.no_fixtures')}
        </p>
      )}
    </Card>
  );
}

import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { calendarHtml, CALENDAR_LAYOUT_KEY, readCalendarLayout, type CalendarViewState } from './calendar';
import { useAutoRefresh } from './useAutoRefresh';
import { PersonMergeDialog } from './PersonMergeDialog';
import { PersonExclusions } from './PersonExclusions';
import { defaultGitConfig, type GitActivityConfig, type GitActivityReport, type GitSource } from './types';
import './git-activity.css';

export function GitActivityView() {
  const [config, setConfig] = useState<GitActivityConfig>(defaultGitConfig);
  const [report, setReport] = useState<GitActivityReport | null>(null);
  const [savedConfig, setSavedConfig] = useState<GitActivityConfig | null>(null);
  const [finishedAt, setFinishedAt] = useState(0);
  const runningRef = useRef(false);
  const iframe = useRef<HTMLIFrameElement>(null);
  const viewState = useRef<CalendarViewState>(readCalendarLayout());
  const [kind, setKind] = useState<GitSource['kind']>('local');
  const [location, setLocation] = useState('');
  const [aliasText, setAliasText] = useState('');
  const [excludedText, setExcludedText] = useState('');
  const [loading, setLoading] = useState(true);
  const [running, setRunning] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [mergeAliases, setMergeAliases] = useState<Record<string, string> | null>(null);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [html, setHtml] = useState('');
  const desktop = isTauri();
  const applyConfig = (next: GitActivityConfig) => {
    setConfig(next);
    setAliasText(Object.entries(next.aliases).map(([email, name]) => `${email} = ${name}`).join('\n'));
    setExcludedText(next.excludedAuthors.join('\n'));
  };

  function showReport(next: GitActivityReport) {
    setReport(next);
    setHtml(calendarHtml(next, viewState.current));
  }
  useEffect(() => {
    const receive = (event: MessageEvent) => {
      if (event.source !== iframe.current?.contentWindow || event.data?.type !== 'git-calendar-view') return;
      const value = event.data.view;
      if (!value || !['month', 'repo', 'who', 'heat'].every(key => typeof value[key] === 'string') || typeof value.bulk !== 'boolean' || !Number.isFinite(value.scrollY)) return;
      if (!['auto', '1', '2', '3'].includes(value.columns) || !['comfortable', 'compact'].includes(value.density)) return;
      if (viewState.current.columns !== value.columns || viewState.current.density !== value.density) {
        try { localStorage.setItem(CALENDAR_LAYOUT_KEY, JSON.stringify({columns: value.columns, density: value.density})); } catch { /* Keep the session preference. */ }
      }
      viewState.current = {columns: value.columns, density: value.density, month: value.month, repo: value.repo, who: value.who, heat: value.heat, bulk: value.bulk, scrollY: Math.max(0, value.scrollY)};
    };
    window.addEventListener('message', receive);
    return () => window.removeEventListener('message', receive);
  }, []);
  useEffect(() => {
    let active = true;
    if (!desktop) { setLoading(false); return; }
    void invoke<{config: GitActivityConfig; report: GitActivityReport | null}>('get_git_activity')
      .then(saved => { if (active) { applyConfig(saved.config); setSavedConfig(saved.config); if (saved.report) showReport(saved.report); } })
      .catch(e => { if (active) setError(String(e)); })
      .finally(() => { if (active) setLoading(false); });
    const subscription = listen<string>('git-activity-progress', e => { if (active) setMessage(e.payload); });
    void subscription.catch(e => { if (active) setError(String(e)); });
    return () => { active = false; void subscription.then(unlisten => unlisten()).catch(() => {}); };
  }, [desktop]);

  function addSource(value = location, sourceKind = kind) {
    const trimmed = value.trim();
    if (!trimmed) { setError('请先填写仓库路径或地址。'); return; }
    if (config.sources.some(s => s.kind === sourceKind && s.location === trimmed)) { setError('此来源已添加。'); return; }
    setConfig(c => ({...c, sources: [...c.sources, {kind: sourceKind, location: trimmed}]}));
    setLocation(''); setError('');
  }
  async function chooseFolder() {
    try { const path = await invoke<string | null>('select_git_activity_folder'); if (path) addSource(path, 'local'); }
    catch(e) { setError(String(e)); }
  }
  async function importConfig() {
    try { const value = await invoke<GitActivityConfig | null>('import_git_activity_config'); if (value) { applyConfig(value); setError(''); setSettingsOpen(true); setMessage('配置已导入，点击“生成 / 更新日历”开始统计。'); } }
    catch(e) { setError(String(e)); }
  }
  function readAliases() {
    const aliases: Record<string,string> = Object.create(null);
    for (const line of aliasText.split('\n').map(s => s.trim()).filter(Boolean)) {
      const at = line.indexOf('=');
      if (at <= 0 || !line.slice(at+1).trim()) { setError('账号合并每行填写：邮箱 = 显示名。同一人的多个邮箱填写相同显示名。'); return; }
      const email = line.slice(0,at).trim().toLowerCase();
      if (Object.hasOwn(aliases, email)) { setError(`邮箱重复：${email}`); return; }
      aliases[email] = line.slice(at+1).trim();
    }
    return aliases;
  }
  function openMerge() {
    const aliases = readAliases();
    if (aliases) { setError(''); setMergeAliases(aliases); }
  }
  function mergePeople(emails: string[], name: string) {
    const aliases = {...mergeAliases};
    for (const email of emails) aliases[email] = name;
    setAliasText(Object.entries(aliases).map(([email, displayName]) => `${email} = ${displayName}`).join('\n'));
    setMergeAliases(null);
    void run(aliases);
  }
  async function run(mergedAliases?: Record<string, string>, automatic = false) {
    if (runningRef.current) return false;
    const aliases = automatic ? savedConfig?.aliases : mergedAliases ?? readAliases();
    if (!aliases) return false;
    const next = automatic ? savedConfig : {...config, aliases, excludedAuthors: excludedText.split('\n').map(s => s.trim()).filter(Boolean)};
    if (!next?.sources.length) return false;
    runningRef.current = true;
    if (!automatic) { applyConfig(next); setSavedConfig(next); }
    setError(''); setMessage(automatic ? '正在自动更新 Git 提交记录……' : '准备读取 Git 仓库……'); setRunning(true);
    try { showReport(await invoke<GitActivityReport>('run_git_activity', {config: next})); if (!automatic) setSettingsOpen(false); setMessage(''); }
    catch(e) { setError(String(e)); setMessage('本次更新未完成，上一份日历仍然保留。'); }
    finally { runningRef.current = false; setRunning(false); setFinishedAt(Date.now()); }
    return true;
  }
  const dirty = savedConfig !== null && (JSON.stringify(config) !== JSON.stringify(savedConfig)
    || aliasText !== Object.entries(savedConfig.aliases).map(([email, name]) => `${email} = ${name}`).join('\n')
    || excludedText !== savedConfig.excludedAuthors.join('\n') || !!location.trim());
  const paused = running || settingsOpen || mergeAliases !== null || dirty;
  const { preferences, updatePreferences } = useAutoRefresh({
    ready: desktop && !loading && savedConfig !== null,
    hasSources: !!savedConfig?.sources.length,
    paused, finishedAt, refresh: () => run(undefined, true),
  });
  async function exportHtml() {
    try { const path = await invoke<string | null>('export_git_activity_html', {html: report ? calendarHtml(report, viewState.current) : html}); if (path) setMessage(`日历已保存：${path}`); }
    catch(e) { setError(String(e)); }
  }
  return <div className="gitActivityPage">
    <header className="gitActivityHeading">
      <div className="gitActivityTitle"><h1>代码统计</h1><small>{preferences.minutes > 0 ? `每 ${preferences.minutes} 分钟自动更新${paused && !running ? ' · 编辑时暂停' : ''}` : preferences.onOpen ? '打开时自动更新' : '手动更新'}</small></div>
      <div className="gitActivityActions">
        {report && <button type="button" disabled={!desktop || loading || running} onClick={openMerge}>人员合并</button>}
        {report && <button type="button" aria-expanded={settingsOpen} aria-controls="gitActivitySettings" onClick={() => setSettingsOpen(open => !open)}>仓库设置</button>}
        <button type="button" onClick={() => void importConfig()} disabled={!desktop || loading || running}>导入配置</button>
        {report && <button type="button" disabled={!desktop || running} onClick={() => void exportHtml()}>保存独立 HTML</button>}
        <button className="primary" type="button" disabled={!desktop || loading || running || !config.sources.length} onClick={() => void run()}>{running ? '正在更新…' : '生成 / 更新日历'}</button>
      </div>
    </header>
    {mergeAliases && report && <PersonMergeDialog people={report.people} aliases={mergeAliases} onClose={() => setMergeAliases(null)} onMerge={mergePeople} />}
    {!desktop && <div className="gitActivityNotice">请使用桌面版选择本地目录和读取 Git；浏览器预览不会访问你的文件。</div>}
    <section id="gitActivitySettings" className="gitActivitySources" aria-label="仓库来源" hidden={!!report && !settingsOpen}>
      <div className="gitAutoSettings">
        <label><input type="checkbox" checked={preferences.onOpen} onChange={event => updatePreferences({...preferences, onOpen: event.target.checked})} />打开程序时自动更新</label>
        <label>自动更新间隔<select value={preferences.minutes} onChange={event => updatePreferences({...preferences, minutes: Number(event.target.value)})}><option value={0}>关闭定时更新</option><option value={5}>每 5 分钟</option><option value={15}>每 15 分钟</option><option value={30}>每 30 分钟</option><option value={60}>每 60 分钟</option></select></label>
        <p className="gitActivityHelp">设置立即保存。程序运行期间自动更新，退出后停止；编辑设置时暂停，失败保留原日历。是否获取远程新提交遵循下方同步设置。</p>
      </div>
      <h2>添加仓库</h2>
      <fieldset disabled={loading || running}>
        <div className="gitActivitySourceInput">
          <label>来源<select aria-label="来源类型" value={kind} onChange={e => setKind(e.target.value as GitSource['kind'])}><option value="local">本地目录</option><option value="remote">远程 Git 地址</option></select></label>
          <label className="gitActivityLocation">{kind === 'local' ? '仓库或项目集合目录' : 'Git 克隆地址'}<input aria-label="仓库路径" value={location} onChange={e => setLocation(e.target.value)} onKeyDown={e => { if(e.key === 'Enter') addSource(); }} placeholder={kind === 'local' ? '例如 D:\\projects，可自动发现多个仓库' : 'https://example.com/team/repo.git 或 git@host:team/repo.git'} /></label>
          {kind === 'local' && <button type="button" onClick={() => void chooseFolder()} disabled={!desktop}>选择文件夹</button>}
          <button type="button" onClick={() => addSource()}>添加</button>
        </div>
        <p className="gitActivityHelp">支持多次添加。远程仓库使用本机 Git 凭据或 SSH 密钥；请输入具体仓库地址。</p>
        {config.sources.length ? <ul className="gitActivitySourceList">{config.sources.map((source,index) => <li key={`${source.kind}:${source.location}`}><span className="gitActivitySourceKind">{source.kind === 'local' ? '本地' : '远程'}</span><code>{source.location}</code><button type="button" aria-label={`移除 ${source.location}`} onClick={() => setConfig(c => ({...c, sources: c.sources.filter((_,i) => i !== index)}))}>移除</button></li>)}</ul> : <div className="gitActivityEmptySources">尚未添加仓库。可以选择单个项目，也可以选择包含多个项目的文件夹。</div>}
        <details className="gitActivityRules" open><summary>排除人员 · 不参与统计</summary>
          <PersonExclusions report={report} value={excludedText} onChange={setExcludedText} onApply={() => void run()} canApply={desktop && !loading && !running && config.sources.length > 0} />
        </details>
        <details className="gitActivityRules"><summary>统计规则 · 账号合并与工作日</summary>
          <div className="gitActivityRuleGrid">
            <label>账号合并 / 重命名<textarea value={aliasText} onChange={e => setAliasText(e.target.value)} placeholder={'alice@company.com = Alice\nalice@example.com = Alice'} rows={5}/><small>每行“邮箱 = 显示名”；同一显示名的邮箱会合并。</small></label>
            <label>每周休息日<select value={config.restDays.join(',')} onChange={e => setConfig(c => ({...c, restDays: e.target.value ? e.target.value.split(',').map(Number) : []}))}><option value="0">周日单休</option><option value="0,6">周六、周日双休</option><option value="">无固定休息日</option></select><small>中国节假日、调休优先；未核对年份不进行低行数判定。</small></label>
            <label>工作日低行数提醒<input type="number" min={0} max={100000000} value={config.threshold} onChange={e => setConfig(c => ({...c, threshold: Math.max(0, Math.min(100000000, Math.floor(Number(e.target.value)||0)))}))}/><small>新增＋删除小于此值提示；0 表示关闭，休息日和节假日免判定。</small></label>
          </div>
        </details>
        <div className="gitActivityRun"><label><input type="checkbox" checked={config.syncRemote} onChange={e => setConfig(c => ({...c, syncRemote: e.target.checked}))}/> 统计前同步远程分支和标签</label></div>
      </fieldset>
    </section>
      {running && <div className="gitActivityProgress" role="status"><span className="gitActivitySpinner"/><span>{message}</span><button type="button" onClick={() => void invoke('cancel_git_activity').catch(e => setError(String(e)))}>取消</button></div>}
      {!running && message && <p className="gitActivityHelp" role="status">{message}</p>}
      {error && <p className="gitActivityError" role="alert">{error}</p>}
    <section className="gitActivityResults" aria-label="统计结果">
      {report ? <>
        {report.warnings.length > 0 && <details className="gitActivityWarnings"><summary>统计提示（{report.warnings.length}）</summary>{report.warnings.map(w => <p className="gitActivityNotice" key={w}>{w}</p>)}</details>}
        <iframe ref={iframe} title="每日提交日历" className="gitActivityCalendar" sandbox="allow-scripts allow-downloads allow-popups allow-popups-to-escape-sandbox" srcDoc={html}/></>
        : <div className="gitActivityEmpty"><div aria-hidden="true">▦</div><h3>{loading ? '正在读取上次统计…' : '从仓库到每日贡献，一目了然'}</h3><p>添加仓库并生成日历后，可按年份、月份、人员和仓库筛选，点击日期查看提交详情。</p><p>统计数据保存在本机；提交次数与行数不单独代表工作质量。</p></div>}
    </section>
  </div>;
}

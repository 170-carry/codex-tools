import { useMemo, useState } from 'react';
import type { GitActivityReport } from './types';

type Author = { name: string; email: string };
const normalize = (value: string) => value.trim().toLowerCase();
const matchesRule = (author: Author, rule: string) => normalize(author.name) === normalize(rule) || (author.email.trim() !== '' && normalize(author.email) === normalize(rule));

export function PersonExclusions({ report, value, onChange, onApply, canApply }: {
  report: GitActivityReport | null;
  value: string;
  onChange: (value: string) => void;
  onApply: () => void;
  canApply: boolean;
}) {
  const [search, setSearch] = useState('');
  const [manual, setManual] = useState('');
  const rules = value.split('\n').map(rule => rule.trim()).filter(Boolean);
  const people = useMemo(() => {
    const groups = new Map<string, { id: string; name: string; members: Author[] }>();
    const aliases = new Map(Object.entries(report?.config.aliases ?? {}).map(([email, name]) => [normalize(email), name]));
    // Older reports did not keep the excluded authors; known rules remain editable below.
    const authors = report?.authors?.length ? report.authors : report?.rows.map(row => ({name: row.author, email: row.email})) ?? [];
    for (const author of authors) {
      const alias = aliases.get(normalize(author.email));
      const id = alias ? `alias:${alias}` : author.email.trim() ? `email:${normalize(author.email)}` : `name:${author.name}`;
      let group = groups.get(id);
      if (!group) {
        group = {id, name: alias || author.name || author.email, members: []};
        groups.set(id, group);
      }
      if (!group.members.some(member => member.name === author.name && normalize(member.email) === normalize(author.email))) group.members.push(author);
    }
    return [...groups.values()].sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'));
  }, [report]);
  function changeRules(next: string[]) {
    const unique = new Map(next.map(rule => [normalize(rule), rule.trim()] as const).filter(([key]) => key));
    onChange([...unique.values()].join('\n'));
  }
  function addManual() {
    changeRules([...rules, ...manual.split(/\r?\n/)]);
    setManual('');
  }
  const query = search.trim().toLowerCase();
  const visible = people.filter(person => [person.name, ...person.members.flatMap(member => [member.name, member.email])].join(' ').toLowerCase().includes(query));
  const pending = JSON.stringify([...new Set(rules.map(normalize))].sort()) !== JSON.stringify([...new Set((report?.config.excludedAuthors ?? []).map(normalize))].sort());

  return <div className="gitExclusions">
    <p className="gitActivityHelp">勾选不需要统计的人员，合并账号会一起排除。取消勾选可恢复。修改后点击下方按钮保存并更新。</p>
    {people.length > 0 && <>
      <label>查找排除人员<input type="search" value={search} onChange={event => setSearch(event.target.value)} placeholder="姓名或邮箱" /></label>
      <div className="gitMergeList" role="group" aria-label="人员排除名单">
        {visible.map(person => {
          const excludedCount = person.members.filter(member => rules.some(rule => matchesRule(member, rule))).length;
          const checked = excludedCount === person.members.length;
          return <label className="gitMergePerson" key={person.id}>
            <input type="checkbox" checked={checked} ref={element => { if (element) element.indeterminate = excludedCount > 0 && !checked; }} aria-label={`排除 ${person.name}`} onChange={event => changeRules(event.target.checked
              ? [...rules, ...person.members.map(member => member.email.trim() || member.name)]
              : rules.filter(rule => !person.members.some(member => matchesRule(member, rule))))} />
            <span><strong>{person.name} <em>{checked ? '已勾选排除' : excludedCount ? '部分账号已排除' : '参与统计'}</em></strong><small>{[...new Set(person.members.map(member => member.email || member.name))].join(' · ')}</small></span>
          </label>;
        })}
        {!visible.length && <p className="gitActivityHelp">没有匹配的人员。</p>}
      </div>
    </>}
    {!report?.authors?.length && <p className="gitActivityHelp">更新一次日历后，这里会列出完整人员名单，包括之前已排除的人。</p>}
    <div className="gitExclusionManual"><label>手动排除姓名或邮箱<input value={manual} onChange={event => setManual(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); addManual(); } }} placeholder="例如 libin 或 user@example.com" /></label><button type="button" disabled={!manual.trim()} onClick={addManual}>加入排除</button></div>
    <p className="gitActivityHelp">按邮箱精确排除；手动填写原始姓名会匹配所有同名作者，忽略大小写。</p>
    {rules.length > 0 && <div className="gitExclusionRules" aria-label="已配置的排除规则">{rules.map((rule, index) => <span key={`${rule}:${index}`}><span>{rule}</span><button type="button" aria-label={`恢复 ${rule}`} onClick={() => changeRules(rules.filter(item => normalize(item) !== normalize(rule)))}>恢复</button></span>)}</div>}
    <div className="gitExclusionApply"><span className="gitActivityHelp" role="status">{pending ? '有未应用的排除设置' : rules.length ? `已配置 ${rules.length} 条排除规则` : '未设置排除人员'}</span><button className="primary" type="button" disabled={!canApply} onClick={onApply}>应用排除并更新日历</button></div>
  </div>;
}

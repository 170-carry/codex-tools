import { useEffect, useRef, useState } from 'react';
import type { GitActivityReport } from './types';

type Props = {
  people: GitActivityReport['people'];
  aliases: Record<string, string>;
  onClose: () => void;
  onMerge: (emails: string[], name: string) => void;
};

export function PersonMergeDialog({ people, aliases, onClose, onMerge }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [search, setSearch] = useState('');
  const [name, setName] = useState('');
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  const chosen = people.filter(person => selected.includes(person.identity));
  const emails = [...new Set(chosen.flatMap(person => person.emails).map(email => email.trim().toLowerCase()))];
  // Include configured addresses from the same groups even if absent from the latest report.
  const previousNames = new Set(emails.map(email => aliases[email]).filter(Boolean));
  for (const [email, alias] of Object.entries(aliases)) {
    if (previousNames.has(alias) && !emails.includes(email)) emails.push(email);
  }
  const extraEmails = Object.entries(aliases).filter(([email, alias]) => alias === name.trim() && !emails.includes(email)).map(([email]) => email);
  const query = search.trim().toLowerCase();
  const matches = people.filter(person => [person.person, ...person.names, ...person.emails].join(' ').toLowerCase().includes(query));

  return <dialog ref={dialog} className="gitMergeDialog" aria-labelledby="gitMergeTitle" onCancel={onClose}>
    <form onSubmit={event => { event.preventDefault(); if (chosen.length >= 2 && name.trim()) onMerge(emails, name.trim()); }}>
      <div className="gitMergeHeading"><h2 id="gitMergeTitle">人员合并</h2><button type="button" onClick={onClose} aria-label="关闭人员合并">关闭</button></div>
      <p className="gitActivityHelp">勾选同一个人的多个账号，合并到一张日历。这里列出全部历史人员，包含本月没有提交的账号。</p>
      <label>查找账号<input type="search" value={search} onChange={event => setSearch(event.target.value)} placeholder="姓名或邮箱" /></label>
      <div className="gitMergeList" role="group" aria-label="待合并账号">
        {matches.map(person => <label className="gitMergePerson" key={person.identity}>
          <input type="checkbox" checked={selected.includes(person.identity)} disabled={!person.emails.length || person.emails.some(email => !email.trim())} onChange={event => setSelected(ids => event.target.checked ? [...ids, person.identity] : ids.filter(id => id !== person.identity))} />
          <span><strong>{person.person}</strong><small>{person.emails.filter(Boolean).join(' · ') || '未记录邮箱，暂不支持合并'}</small></span>
        </label>)}
        {!matches.length && <p className="gitActivityHelp">没有匹配的账号。</p>}
      </div>
      <p className="gitMergeSelection">已选 {chosen.length} 个账号{chosen.length > 0 && `：${chosen.map(person => person.person).join('、')}`}</p>
      <label>合并后姓名<input required maxLength={120} value={name} onChange={event => setName(event.target.value)} placeholder="例如 张三" /></label>
      {extraEmails.length > 0 && <p className="gitActivityNotice">此姓名已有合并规则，以下邮箱也会归入同一人：{extraEmails.join('、')}</p>}
      <p className="gitActivityHelp">合并后重新读取仓库并去重，保留原始作者和邮箱。规则会保存，后续更新自动应用；需要拆分时，可在仓库设置的“账号合并 / 重命名”中修改。</p>
      <div className="gitMergeActions"><button type="button" onClick={onClose}>取消</button><button className="primary" type="submit" disabled={chosen.length < 2 || !name.trim()}>合并并更新日历</button></div>
    </form>
  </dialog>;
}

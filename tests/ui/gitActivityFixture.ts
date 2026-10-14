import { defaultGitConfig, type GitActivityReport } from '../../src/features/git-activity/types';

export const gitActivityFixture: GitActivityReport = {
  start: '2025-12-01', end: '2026-10-14', snapshot: '2026-10-14T14:00:00+08:00',
  config: {...defaultGitConfig, sources: [{kind:'local', location:'D:\\demo-repository'}]},
  warnings: [],
  authors: [{name:'Alice',email:'alice@example.com'},{name:'Bob',email:'bob@example.com'}],
  people: [
    {identity:'alice@example.com',person:'Alice',names:['Alice'],emails:['alice@example.com'],company_unique:2},
    {identity:'bob@example.com',person:'Bob',names:['Bob'],emails:['bob@example.com'],company_unique:1},
  ],
  repos: [{key:'demo',repo:'示例项目',category:'selected',synced:true,shallow:false}],
  rows: [
    {sha:'a'.repeat(40),repo_key:'demo',person:'alice@example.com',author:'Alice',email:'alice@example.com',subject:'fix: </script><script>window.pwned=true</script>',commit_day:'2026-10-08',source_add:499,source_del:0,source_files:1,bulk:false,keep:true,company_unique:true,all_unique:true},
    {sha:'b'.repeat(40),repo_key:'demo',person:'alice@example.com',author:'Alice',email:'alice@example.com',subject:'feat: second change',commit_day:'2026-10-09',source_add:400,source_del:100,source_files:2,bulk:false,keep:true,company_unique:true,all_unique:true},
    {sha:'c'.repeat(40),repo_key:'demo',person:'bob@example.com',author:'Bob',email:'bob@example.com',subject:'initial',commit_day:'2025-12-01',source_add:1,source_del:0,source_files:1,bulk:false,keep:true,company_unique:true,all_unique:true},
  ],
};

export const gitLayoutFixture: GitActivityReport = {
  ...gitActivityFixture,
  people: [
    gitActivityFixture.people[0],
    {...gitActivityFixture.people[1], company_unique: 2},
    {identity:'carol@example.com',person:'Carol',names:['Carol'],emails:['carol@example.com'],company_unique:1},
    {identity:'dave@example.com',person:'Dave',names:['Dave'],emails:['dave@example.com'],company_unique:1},
  ],
  authors: [...gitActivityFixture.authors!, {name:'Carol',email:'carol@example.com'},{name:'Dave',email:'dave@example.com'}],
  rows: [...gitActivityFixture.rows, ...['Bob','Carol','Dave'].map((name,index) => ({...gitActivityFixture.rows[0], sha: String(index+1).repeat(40), person: `${name.toLowerCase()}@example.com`, author: name, email: `${name.toLowerCase()}@example.com`, subject: 'Layout fixture', source_add: 124672}))],
};

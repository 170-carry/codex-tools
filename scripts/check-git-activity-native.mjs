// Opt-in smoke check against a separately launched debug desktop app and real Git sources.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
const local = process.env.CODEX_TOOLS_GIT_TEST_LOCAL_PATH;
const remote = process.env.CODEX_TOOLS_GIT_TEST_REMOTE_URL;
if (!local || !remote) throw new Error('Set CODEX_TOOLS_GIT_TEST_LOCAL_PATH and CODEX_TOOLS_GIT_TEST_REMOTE_URL to authorized test repositories.');
const modulePath = process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE;
const { chromium } = await import(modulePath ? pathToFileURL(modulePath).href : 'playwright');
const browser = await chromium.connectOverCDP('http://127.0.0.1:9227');
const page = browser.contexts()[0].pages()[0];
const output = path.resolve('.codex-verification/git-activity-native');
await mkdir(output,{recursive:true});
try {
  const onboarding = page.getByRole('button',{name:'完成设置',exact:true});
  if(await onboarding.isVisible()) await onboarding.click();
  await page.locator('.viewTab,.topSegmentedButton').filter({hasText:'代码统计'}).click();
  await page.getByRole('heading',{name:'代码统计',exact:true}).waitFor();
  if (!(await page.getByLabel('仓库路径').isVisible())) await page.getByRole('button',{name:'仓库设置',exact:true}).click();
  await page.getByLabel('仓库路径').fill(local);
  await page.getByRole('button',{name:'添加',exact:true}).click();
  await page.getByLabel('来源类型').selectOption('remote');
  await page.getByLabel('仓库路径').fill(remote);
  await page.getByRole('button',{name:'添加',exact:true}).click();
  await page.getByRole('button',{name:'生成 / 更新日历',exact:true}).click();
  await Promise.race([page.locator('.gitActivityCalendar').waitFor({timeout:300000}),page.locator('.gitActivityError').waitFor({timeout:300000})]);
  if(await page.locator('.gitActivityError').isVisible()) throw new Error(await page.locator('.gitActivityError').innerText());
  const summary=await page.evaluate(async()=>{
    const saved=await window.__TAURI_INTERNALS__.invoke('get_git_activity');
    const r=saved.report;
    return {snapshot:r.snapshot,repos:r.repos.length,people:r.people.length,commits:r.rows.filter(c=>c.company_unique).length,sources:saved.config.sources.map(s=>s.kind),warnings:r.warnings};
  });
  assert(summary.commits>0);assert.deepEqual(summary.sources,['local','remote']);
  assert.equal(summary.repos,1,'local and remote clones with the same origin group together');
  const frame=page.frameLocator('.gitActivityCalendar');
  await frame.locator('#grid').waitFor();
  // An update of a missing folder must fail without replacing the successful report.
  const failure=await page.evaluate(async()=>{
    const saved=await window.__TAURI_INTERNALS__.invoke('get_git_activity');
    try {await window.__TAURI_INTERNALS__.invoke('run_git_activity',{config:{...saved.config,sources:[{kind:'local',location:'Z:\\__codex_nonexistent_git_fixture__'}]}});return false;}catch{return (await window.__TAURI_INTERNALS__.invoke('get_git_activity')).report.snapshot===saved.report.snapshot;}
  });
  assert(failure,'native failed update preserves last snapshot');
  await page.getByRole('heading',{name:'代码统计',exact:true}).scrollIntoViewIfNeeded();
  await page.screenshot({path:path.join(output,'desktop.png')});
  await writeFile(path.join(output,'checks.json'),JSON.stringify({...summary,failedUpdatePreservesReport:failure},null,2));
  console.log(JSON.stringify({...summary,failedUpdatePreservesReport:failure},null,2));
}finally{await browser.close()}
